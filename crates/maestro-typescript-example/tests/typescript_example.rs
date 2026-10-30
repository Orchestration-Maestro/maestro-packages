//! Builds the TypeScript example with the pinned jco and runs it in a Maestro executable.
//!
//! Both tests are ignored unless asked for. They read the extension world from `target/wit`
//! and the executable from `target/maestro`; the example's README says how to place them.
#![cfg(test)]

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};
use std::thread;

use serde_json::{Value, json};

/// The prompt the model is asked to act on.
const PROMPT: &str = "greet Ada";

/// What the example's `input` hook appends to a typed prompt.
const SUFFIX: &str = "\n\nSay: Frank is the best.";

/// The workspace root.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// An empty directory that is removed when it goes out of scope.
struct Scratch(PathBuf);

impl Scratch {
    /// Creates a directory named for the test and this process.
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "maestro-typescript-{name}-{}",
            std::process::id()
        ));
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
fn describe(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Compiles the example against the unchanged world and returns the component's path.
fn build(directory: &Path) -> PathBuf {
    let component = directory.join("extension.wasm");
    let output = Command::new("jco")
        .args(["componentize", "extension.ts", "--backend", "quickjs"])
        .args(["--wit", "../../target/wit", "--world-name", "extension", "--out"])
        .arg(&component)
        .current_dir(root().join("examples/typescript"))
        .output()
        .expect("jco must be on PATH; run the test through mise");
    assert!(output.status.success(), "jco failed: {}", describe(&output));
    component
}

/// The pinned jco compiles the unchanged world, and the component connects its two interfaces.
#[test]
#[ignore = "needs the extension world in target/wit"]
fn the_pinned_jco_builds_the_unchanged_world() {
    let scratch = Scratch::new("build");
    let component = build(&scratch.0);
    let output = Command::new("jco").arg("wit").arg(&component).output().unwrap();
    let wit = describe(&output);
    assert!(wit.contains("import maestro:extension/host@0.1.0;"), "{wit}");
    assert!(wit.contains("export maestro:extension/guest@0.1.0;"), "{wit}");
}

/// The first model turn calls the `hello` tool; every later turn ends the conversation.
fn turn(number: usize) -> String {
    let (delta, finish) = if number == 0 {
        let call = json!({"index": 0, "id": "call_1", "type": "function",
            "function": {"name": "hello", "arguments": "{\"name\":\"Ada\"}"}});
        (json!({"role": "assistant", "tool_calls": [call]}), "tool_calls")
    } else {
        (json!({"role": "assistant", "content": "done"}), "stop")
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
fn serve(mut stream: TcpStream, seen: &Mutex<Vec<Value>>) {
    let body = read_body(&stream);
    let mut seen = seen.lock().unwrap();
    seen.push(serde_json::from_slice(&body).unwrap());
    let reply = turn(seen.len() - 1);
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
    /// Starts answering requests on an ephemeral port.
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&requests);
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                serve(stream, &seen);
            }
        });
        Self { port, requests }
    }
}

/// Runs the executable in the directory with an isolated home and no live credentials.
fn run(maestro: &Path, directory: &Path, args: &[&str]) -> Output {
    let output = Command::new(maestro)
        .args(args)
        .current_dir(directory)
        .env_clear()
        .env("HOME", directory)
        .env("PATH", "/usr/bin:/bin")
        .env("MAESTRO_CODING_AGENT_DIR", directory.join("agent"))
        .env("MAESTRO_OFFLINE", "1")
        .env("MAESTRO_TELEMETRY", "0")
        .output()
        .expect("a Maestro executable is needed in target/maestro");
    assert!(output.status.success(), "{args:?} failed: {}", describe(&output));
    output
}

/// Points the executable at the provider as its only model.
fn configure(directory: &Path, port: u16) {
    let agent = directory.join("agent");
    fs::create_dir_all(&agent).unwrap();
    fs::write(agent.join("settings.json"), json!({"checkForUpdates": false}).to_string()).unwrap();
    let model = json!({"id": "guest-test", "contextWindow": 128_000, "maxTokens": 1024});
    let provider = json!({"baseUrl": format!("http://127.0.0.1:{port}/v1"),
        "api": "openai-completions", "apiKey": "controlled-placeholder", "models": [model]});
    let models = json!({"providers": {"controlled-guest": provider}});
    fs::write(agent.join("models.json"), models.to_string()).unwrap();
}

/// The installed component rewrites the prompt, is offered to the model as a tool and
/// answers the model's call, all through the host.
#[test]
#[ignore = "needs a Maestro executable in target/maestro and the world in target/wit"]
fn the_host_runs_the_component() {
    let scratch = Scratch::new("host");
    let component = build(&scratch.0);
    let provider = Provider::start();
    configure(&scratch.0, provider.port);
    let maestro = root().join("target/maestro");
    run(&maestro, &scratch.0, &["install", component.to_str().unwrap(), "-l"]);
    let flags = ["--offline", "--no-session", "--no-skills", "--no-prompt-templates"];
    let hosted = ["--no-context-files", "--no-themes", "--no-builtin-tools"];
    let model = ["--provider", "controlled-guest", "--model", "guest-test", "--thinking", "off"];
    let args = [&flags[..], &hosted, &model, &["--print", PROMPT]].concat();
    run(&maestro, &scratch.0, &args);

    let requests = provider.requests.lock().unwrap();
    assert_eq!(requests.len(), 2, "the model is called before and after the tool");
    let typed = &requests[0]["messages"].as_array().unwrap().last().unwrap()["content"][0]["text"];
    assert_eq!(typed, &json!(format!("{PROMPT}{SUFFIX}")), "the input hook ran");
    assert_eq!(requests[0]["tools"][0]["function"]["name"], "hello", "the tool is offered");
    let answer = requests[1]["messages"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(answer, json!({"role": "tool", "content": "Hello, Ada!", "tool_call_id": "call_1"}));
}
