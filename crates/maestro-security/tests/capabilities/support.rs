//! Isolated model and runner fixtures; each case runs in its own process.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use maestro_test_extension_host::{describe, read_request};
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
    /// Command supplied by the model that the runner must execute.
    command: Option<&'static str>,
}

impl Reply {
    /// Return a model completion containing the supplied answer.
    pub fn chat(answer: &str) -> Self {
        Self {
            command: None,
            path: "/chat/completions",
            status: 200,
            body: json!({"choices":[{"message":{"content":answer}}]}).to_string(),
        }
    }

    /// Return the runner's observed output and process status.
    pub fn exec(output: &str, code: i32) -> Self {
        Self {
            command: None,
            path: "/exec",
            status: 200,
            body: format!(
                "{}\n{}\n",
                json!({"type":"out", "data":output}),
                json!({"type":"exit", "code":code})
            ),
        }
    }

    /// Require the runner to execute the model's proposed command unchanged.
    pub fn command(mut self, command: &'static str) -> Self {
        self.command = Some(command);
        self
    }

    /// Simulate a service rejecting a request.
    pub fn failure(path: &'static str) -> Self {
        Self {
            path,
            status: 403,
            body: "fixture unavailable".into(),
            command: None,
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
    let (completed, completion) = mpsc::channel();
    let server = std::thread::spawn(move || serve(&listener, replies, &completion));
    let child = Command::new(std::env::current_exe()?)
        .args(["--exact", name, "--nocapture"])
        .env("CAPABILITY_CHILD", name)
        .env("MAESTRO_SECURITY_BRIDGE", &url)
        .env("MAESTRO_SECURITY_RUNNER", &url)
        .env("MAESTRO_SECURITY_BRIDGE_KEY", "fixture-key")
        .output();
    let _ = completed.send(());
    let exchanges = server.join().map_err(|_| "fixture server panicked")?;
    let child = child?;
    assert!(
        child.status.success(),
        "child failed:\n{}",
        describe(&child)
    );
    exchanges
}

/// Observe all requests until child completion, rejecting unscripted exchanges.
fn serve(listener: &TcpListener, replies: Vec<Reply>, completion: &Receiver<()>) -> Result {
    let mut replies = replies.into_iter().peekable();
    'requests: loop {
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(false)?;
                    #[cfg(unix)]
                    assert!(
                        !socket2::SockRef::from(&stream).nonblocking()?,
                        "accepted fixture stream must be blocking"
                    );
                    break stream;
                }
                Err(error) if error.kind() != std::io::ErrorKind::WouldBlock => {
                    return Err(error.into());
                }
                Err(_) if completion.try_recv().is_ok() => break 'requests,
                Err(_) if Instant::now() >= deadline && replies.peek().is_some() => {
                    return Err("timed out waiting for a scripted request".into());
                }
                Err(_) => std::thread::sleep(Duration::from_millis(5)),
            }
        };
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        let request = read_request(&stream)?.text();
        let reply = replies
            .next()
            .ok_or_else(|| format!("unscripted request: {request}"))?;
        assert!(
            request.starts_with(&format!("POST {} HTTP/1.1", reply.path)),
            "{request}"
        );
        if let Some(command) = reply.command {
            let (_, body) = request.split_once("\r\n\r\n").ok_or("missing body")?;
            let value: serde_json::Value = serde_json::from_str(body)?;
            assert_eq!(value["command"], command);
        }
        write!(
            stream,
            "HTTP/1.1 {} OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            reply.status,
            reply.body.len(),
            reply.body
        )?;
    }
    replies.peek().map_or(Ok(()), |reply| {
        Err(format!("missing request to {}", reply.path).into())
    })
}

/// A swallowed runner failure must still fail the parent fixture.
#[test]
fn rejects_unscripted_runner_requests() -> Result {
    let name = "support::rejects_unscripted_runner_requests";
    let result = case(name, vec![Reply::chat("plan")], || {
        let url = std::env::var("MAESTRO_SECURITY_RUNNER")?;
        let address = url.strip_prefix("http://").ok_or("invalid fixture URL")?;
        let mut stream = TcpStream::connect(address)?;
        write!(
            stream,
            "POST /chat/completions HTTP/1.1\r\nContent-Length: 0\r\n\r\n"
        )?;
        stream.read_to_end(&mut Vec::new())?;
        if let Ok(mut runner) = TcpStream::connect(address) {
            let _ = write!(runner, "POST /exec HTTP/1.1\r\nContent-Length: 0\r\n\r\n");
            let _ = runner.read_to_end(&mut Vec::new());
        }
        Ok(())
    });
    if std::env::var("CAPABILITY_CHILD").as_deref() == Ok(name) {
        return result;
    }
    assert!(result.is_err(), "unscripted runner request went unobserved");
    Ok(())
}
