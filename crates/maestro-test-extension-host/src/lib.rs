//! Runs installed components in a Maestro executable against a scripted local model.
//!
//! Nothing here knows the guest language: a caller supplies the compiled components, the
//! prompt and the model's turns, then judges the requests the model saw and the executable's
//! exit and output.
//!
//! The request reader and the output description stand apart from the guest, so the loopback
//! fixtures in other crates' tests use them too.
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;

use serde_json::{Value, json};

pub use maestro_test_directory::Scratch;

/// The workspace root.
#[must_use]
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The standard output and error of a finished command, for assertion messages.
#[must_use]
pub fn describe(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// The last message the model received in a request, or `null` when it received none.
#[must_use]
pub fn last_message(request: &Value) -> &Value {
    request["messages"]
        .as_array()
        .and_then(|messages| messages.last())
        .unwrap_or(&Value::Null)
}

/// The text of the last message the model received in a request.
#[must_use]
pub fn last_text(request: &Value) -> &Value {
    &last_message(request)["content"][0]["text"]
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
fn chunks(turn: &Turn) -> String {
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

/// One request read off a connection.
#[derive(Debug)]
pub struct Request {
    /// The request line and headers, including the blank line that ends them.
    pub head: String,
    /// The body, as long as the `Content-Length` header says (empty without one).
    pub body: Vec<u8>,
}

impl Request {
    /// The whole request as text, with a body that is not UTF-8 replaced lossily.
    #[must_use]
    pub fn text(&self) -> String {
        format!("{}{}", self.head, String::from_utf8_lossy(&self.body))
    }
}

/// Reads one framed request: its headers, then exactly as many body bytes as `Content-Length`
/// announces. Header names match in any letter case.
///
/// # Errors
/// Returns an error when the peer closes before the headers end or before the whole body
/// arrives, when `Content-Length` is not a number, and any read failure.
pub fn read_request(stream: impl Read) -> io::Result<Request> {
    let mut reader = BufReader::new(stream);
    let mut head = String::new();
    let mut length = 0;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "the request ended before its headers did",
            ));
        }
        head.push_str(&line);
        if line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().map_err(io::Error::other)?;
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    Ok(Request { head, body })
}

/// Locks the recorded requests; a panic elsewhere never hides what was already recorded.
fn locked(requests: &Mutex<Vec<Value>>) -> MutexGuard<'_, Vec<Value>> {
    requests.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Records one request and answers it with the next scripted turn.
fn serve(mut stream: TcpStream, script: &[Turn], seen: &Mutex<Vec<Value>>) -> io::Result<()> {
    let body = read_request(&stream)?.body;
    let mut seen = locked(seen);
    seen.push(serde_json::from_slice(&body)?);
    let reply = chunks(script.get(seen.len() - 1).unwrap_or(&CLOSING));
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{reply}",
        reply.len()
    )
}

/// Answers connections in turn until one cannot be served.
fn serve_all(listener: &TcpListener, script: &[Turn], seen: &Mutex<Vec<Value>>) {
    for stream in listener.incoming().flatten() {
        if let Err(error) = serve(stream, script, seen) {
            eprintln!("the scripted model stopped serving: {error}");
            break;
        }
    }
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
    fn start(script: Vec<Turn>) -> io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&requests);
        thread::spawn(move || serve_all(&listener, &script, &seen));
        Ok(Self { port, requests })
    }
}

/// Runs the executable in the directory with an isolated home and no live credentials.
fn execute(maestro: &Path, directory: &Path, args: &[&str]) -> io::Result<Output> {
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
        .map_err(|error| {
            let message = format!("a Maestro executable is needed in target/maestro: {error}");
            io::Error::new(error.kind(), message)
        })
}

/// Points the executable at the provider as its only model.
fn configure(directory: &Path, port: u16) -> io::Result<()> {
    let agent = directory.join("agent");
    fs::create_dir_all(&agent)?;
    fs::write(
        agent.join("settings.json"),
        json!({"checkForUpdates": false}).to_string(),
    )?;
    let model = json!({"id": "guest-test", "contextWindow": 128_000, "maxTokens": 1024});
    let provider = json!({"baseUrl": format!("http://127.0.0.1:{port}/v1"),
        "api": "openai-completions", "apiKey": "controlled-placeholder", "models": [model]});
    let models = json!({"providers": {"controlled-guest": provider}});
    fs::write(agent.join("models.json"), models.to_string())
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
/// # Errors
/// Returns the failure to start the model, to write its settings or to start the executable
/// (which is expected in `target/maestro`), and an install that exits unsuccessfully. The run's
/// own exit status is the caller's to judge.
pub fn run(
    directory: &Path,
    components: &[&Path],
    prompt: &str,
    script: &[Turn],
) -> io::Result<Run> {
    let provider = Provider::start(script.to_vec())?;
    configure(directory, provider.port)?;
    let maestro = root().join("target/maestro");
    for component in components {
        let component = component.to_str().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "non-UTF-8 component path")
        })?;
        let output = execute(&maestro, directory, &["install", component, "-l"])?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "install failed: {}",
                describe(&output)
            )));
        }
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
    let output = execute(&maestro, directory, &args)?;
    let requests = locked(&provider.requests).clone();
    Ok(Run { requests, output })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Posts a JSON body to the provider and returns the whole response text.
    fn post(port: u16, body: &Value) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let body = body.to_string();
        write!(
            stream,
            "POST /v1/chat/completions HTTP/1.1\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    }

    #[test]
    fn provider_records_each_request_and_answers_with_the_next_scripted_turn() {
        let provider = Provider::start(vec![
            Turn::Call {
                tool: "hello",
                arguments: "{\"name\":\"Ada\"}",
            },
            Turn::Say("second"),
        ])
        .unwrap();

        let first = post(provider.port, &json!({"n": 1}));
        let second = post(provider.port, &json!({"n": 2}));
        let third = post(provider.port, &json!({"n": 3}));

        assert!(first.contains("\"name\":\"hello\""), "{first}");
        assert!(second.contains("\"content\":\"second\""), "{second}");
        assert!(third.contains("\"content\":\"done\""), "{third}");
        assert_eq!(
            *locked(&provider.requests),
            [json!({"n": 1}), json!({"n": 2}), json!({"n": 3})]
        );
    }

    /// Hands the bytes over one at a time, as a slow peer does.
    struct Trickle<'a>(&'a [u8]);

    impl Read for Trickle<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let Some((&byte, rest)) = self.0.split_first() else {
                return Ok(0);
            };
            buffer[0] = byte;
            self.0 = rest;
            Ok(1)
        }
    }

    #[test]
    fn read_request_waits_for_the_whole_body_whatever_the_header_casing() {
        for name in ["content-length", "Content-Length", "CONTENT-LENGTH"] {
            let wire = format!("POST /v1 HTTP/1.1\r\n{name}: 4\r\n\r\nbodyextra");

            let request = read_request(Trickle(wire.as_bytes())).unwrap();

            assert_eq!(request.body, b"body", "{name}");
            assert_eq!(
                request.text(),
                format!("POST /v1 HTTP/1.1\r\n{name}: 4\r\n\r\nbody")
            );
        }
    }

    #[test]
    fn read_request_without_a_length_has_an_empty_body() {
        let request = read_request(&b"GET /models HTTP/1.1\r\n\r\n"[..]).unwrap();

        assert_eq!(request.head, "GET /models HTTP/1.1\r\n\r\n");
        assert!(request.body.is_empty());
    }

    #[test]
    fn read_request_fails_when_the_peer_ends_early_or_sends_a_bad_length() {
        let cut_headers = read_request(&b"POST / HTTP/1.1\r\nHost: a\r\n"[..]).unwrap_err();
        let cut_body = read_request(&b"POST / HTTP/1.1\r\nContent-Length: 4\r\n\r\nbo"[..]);
        let bad_length = read_request(&b"POST / HTTP/1.1\r\nContent-Length: x\r\n\r\n"[..]);

        assert_eq!(cut_headers.kind(), io::ErrorKind::UnexpectedEof);
        assert_eq!(
            cut_body.err().map(|e| e.kind()),
            Some(io::ErrorKind::UnexpectedEof)
        );
        assert!(bad_length.is_err());
    }

    #[test]
    fn last_text_is_the_first_text_part_of_the_last_message() {
        let request = json!({"messages": [
            {"content": [{"text": "earlier"}]},
            {"content": [{"text": "latest"}]},
        ]});

        assert_eq!(last_text(&request), &json!("latest"));
    }

    #[test]
    fn last_message_is_null_when_a_request_carries_no_messages() {
        assert_eq!(last_message(&json!({})), &Value::Null);
    }
}
