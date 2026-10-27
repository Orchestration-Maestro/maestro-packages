//! Isolated model and runner fixtures; each case runs in its own process.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;
use std::time::{Duration, Instant};

use serde_json::json;

/// Test and server errors cross the fixture thread boundary.
pub type Result = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

/// One expected service request and its response.
pub struct Reply {
    /// HTTP resource the capability must call.
    path: &'static str,
    /// HTTP response status.
    status: u16,
    /// Wire response body.
    body: String,
}

impl Reply {
    /// Return a model completion containing the supplied answer.
    pub fn chat(answer: &str) -> Self {
        Self {
            path: "/chat/completions",
            status: 200,
            body: json!({"choices":[{"message":{"content":answer}}]}).to_string(),
        }
    }

    /// Return the runner's observed output and process status.
    pub fn exec(output: &str, code: i32) -> Self {
        Self {
            path: "/exec",
            status: 200,
            body: format!(
                "{}\n{}\n",
                json!({"type":"out", "data":output}),
                json!({"type":"exit", "code":code})
            ),
        }
    }
}

/// Run assertions in a child with private service URLs, without mutating the parent environment.
pub fn case(name: &str, replies: Vec<Reply>, check: impl FnOnce() -> Result) -> Result {
    if std::env::var("CAPABILITY_CHILD").as_deref() == Ok(name) {
        return check();
    }
    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let url = format!("http://{}", listener.local_addr()?);
    let server = std::thread::spawn(move || serve(&listener, replies));
    let child = Command::new(std::env::current_exe()?)
        .args(["--exact", name, "--nocapture"])
        .env("CAPABILITY_CHILD", name)
        .env("MAESTRO_SECURITY_BRIDGE", &url)
        .env("MAESTRO_SECURITY_RUNNER", &url)
        .env("MAESTRO_SECURITY_BRIDGE_KEY", "fixture-key")
        .output()?;
    let exchanges = server.join().map_err(|_| "fixture server panicked")?;
    assert!(
        child.status.success(),
        "child failed:\n{}\n{}",
        String::from_utf8_lossy(&child.stdout),
        String::from_utf8_lossy(&child.stderr)
    );
    exchanges
}

/// Serve the scripted exchanges, failing rather than hanging on a missing request.
fn serve(listener: &TcpListener, replies: Vec<Reply>) -> Result {
    for reply in replies {
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() != std::io::ErrorKind::WouldBlock => {
                    return Err(error.into());
                }
                Err(_) if Instant::now() >= deadline => {
                    return Err(format!("missing request to {}", reply.path).into());
                }
                Err(_) => std::thread::sleep(Duration::from_millis(5)),
            }
        };
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        let request = read_request(&mut stream)?;
        assert!(
            request.starts_with(&format!("POST {} HTTP/1.1", reply.path)),
            "{request}"
        );
        write!(
            stream,
            "HTTP/1.1 {} OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            reply.status,
            reply.body.len(),
            reply.body
        )?;
    }
    Ok(())
}

/// Read headers and the complete request body before responding.
fn read_request(
    stream: &mut TcpStream,
) -> std::result::Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let mut bytes = Vec::new();
    let mut buffer = [0; 4096];
    loop {
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            return Err("incomplete HTTP request".into());
        }
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&bytes[..end]);
            let length: usize = headers
                .lines()
                .find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    key.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse().ok())
                        .flatten()
                })
                .ok_or("missing content length")?;
            if bytes.len() >= end + 4 + length {
                return Ok(String::from_utf8(bytes)?);
            }
        }
    }
}
