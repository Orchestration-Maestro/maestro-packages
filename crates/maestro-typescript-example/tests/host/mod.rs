//! Runs installed components in a Maestro executable against a scripted local model.
//!
//! Nothing here knows the guest language: a caller supplies the compiled components, the
//! prompt and the model's turns, then judges the requests the model saw and the executable's
//! exit and output.
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};
use std::thread;

use serde_json::{Value, json};

/// The workspace root.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// An empty directory that is removed when it goes out of scope.
pub struct Scratch(pub PathBuf);

impl Scratch {
    /// Creates a directory named for the test and this process.
    pub fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("maestro-typescript-{name}-{}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The standard output and error of a finished command, for assertion messages.
pub fn describe(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// One scripted model turn.
#[derive(Clone)]
pub enum Turn {
    /// The model answers with text and stops.
    Say(&'static str),
    /// The model calls a tool with a JSON argument document.
    Call {
        /// The tool's registered name.
        tool: &'static str,
        /// The arguments, as JSON text.
        arguments: &'static str,
    },
}

/// What the model answers once the script has run out.
const CLOSING: Turn = Turn::Say("done");

/// The server-sent chunks that carry one turn.
fn stream(turn: &Turn) -> String {
    let (delta, finish) = match turn {
        Turn::Say(text) => (json!({"role": "assistant", "content": text}), "stop"),
        Turn::Call { tool, arguments } => {
            let call = json!({"index": 0, "id": "call_1", "type": "function",
                "function": {"name": tool, "arguments": arguments}});
            (
                json!({"role": "assistant", "tool_calls": [call]}),
                "tool_calls",
            )
        }
    };
    let chunk = |delta: Value, finish: Value| {
        json!({"id": "turn", "object": "chat.completion.chunk", "model": "guest-test",
            "choices": [{"index": 0, "delta": delta, "finish_reason": finish}]})
    };
    format!(
        "data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
        chunk(delta, Value::Null),
        chunk(json!({}), json!(finish))
    )
}

/// Reads one request's headers and body.
fn read_body(stream: &TcpStream) -> Vec<u8> {
    let mut reader = BufReader::new(stream);
    let mut length = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().unwrap();
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).unwrap();
    body
}

/// Records one request and answers it with the next scripted turn.
fn serve(mut stream: TcpStream, script: &[Turn], seen: &Mutex<Vec<Value>>) {
    let body = read_body(&stream);
    let mut seen = seen.lock().unwrap();
    seen.push(serde_json::from_slice(&body).unwrap());
    let reply = self::stream(script.get(seen.len() - 1).unwrap_or(&CLOSING));
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{reply}",
        reply.len()
    )
    .unwrap();
}

/// A model on a loopback port that records every request it receives.
struct Provider {
    /// The port the model listens on.
    port: u16,
    /// The bodies of the requests received so far, oldest first.
    requests: Arc<Mutex<Vec<Value>>>,
}

impl Provider {
    /// Starts answering requests on an ephemeral port, one scripted turn per request.
    fn start(script: Vec<Turn>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&requests);
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                serve(stream, &script, &seen);
            }
        });
        Self { port, requests }
    }
}

/// Runs the executable in the directory with an isolated home and no live credentials.
fn execute(maestro: &Path, directory: &Path, args: &[&str]) -> Output {
    Command::new(maestro)
        .args(args)
        .current_dir(directory)
        .env_clear()
        .env("HOME", directory)
        .env("PATH", "/usr/bin:/bin")
        .env("MAESTRO_CODING_AGENT_DIR", directory.join("agent"))
        .env("MAESTRO_OFFLINE", "1")
        .env("MAESTRO_TELEMETRY", "0")
        .output()
        .expect("a Maestro executable is needed in target/maestro")
}

/// Points the executable at the provider as its only model.
fn configure(directory: &Path, port: u16) {
    let agent = directory.join("agent");
    fs::create_dir_all(&agent).unwrap();
    fs::write(
        agent.join("settings.json"),
        json!({"checkForUpdates": false}).to_string(),
    )
    .unwrap();
    let model = json!({"id": "guest-test", "contextWindow": 128_000, "maxTokens": 1024});
    let provider = json!({"baseUrl": format!("http://127.0.0.1:{port}/v1"),
        "api": "openai-completions", "apiKey": "controlled-placeholder", "models": [model]});
    let models = json!({"providers": {"controlled-guest": provider}});
    fs::write(agent.join("models.json"), models.to_string()).unwrap();
}

/// What a run of the executable produced.
pub struct Run {
    /// The bodies of the requests the model received, oldest first.
    pub requests: Vec<Value>,
    /// The executable's exit status and output.
    pub output: Output,
}

/// Installs each component, then sends the prompt once with the model following the script.
///
/// An install that fails stops the test; the run's own exit status is the caller's to judge.
pub fn run(directory: &Path, components: &[&Path], prompt: &str, script: &[Turn]) -> Run {
    let provider = Provider::start(script.to_vec());
    configure(directory, provider.port);
    let maestro = root().join("target/maestro");
    for component in components {
        let install = ["install", component.to_str().unwrap(), "-l"];
        let output = execute(&maestro, directory, &install);
        assert!(
            output.status.success(),
            "install failed: {}",
            describe(&output)
        );
    }
    let flags = [
        "--offline",
        "--no-session",
        "--no-skills",
        "--no-prompt-templates",
    ];
    let hosted = ["--no-context-files", "--no-themes", "--no-builtin-tools"];
    let model = [
        "--provider",
        "controlled-guest",
        "--model",
        "guest-test",
        "--thinking",
        "off",
    ];
    let args = [&flags[..], &hosted, &model, &["--print", prompt]].concat();
    let output = execute(&maestro, directory, &args);
    let requests = provider.requests.lock().unwrap().clone();
    Run { requests, output }
}
