//! Process-isolated HTTP fixtures for capability behaviour tests.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};

/// A fixture response selected from the request path and JSON body.
pub type Handler = fn(&str, &Value) -> (u16, String);

/// Run the named test in a fresh process with fixture-only endpoints.
/// Returns true in the child, where the caller exercises the capability.
pub fn isolated(name: &str, handler: Handler) -> Result<bool, Box<dyn std::error::Error>> {
    if std::env::var("SECURITY_TEST_CHILD").as_deref() == Ok(name) {
        return Ok(true);
    }
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let url = format!("http://{}", listener.local_addr()?);
    listener.set_nonblocking(true)?;
    let stop = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&stop);
    let server = thread::spawn(move || serve(&listener, &signal, handler));
    let status = Command::new(std::env::current_exe()?)
        .args(["--exact", name, "--nocapture"])
        .env("SECURITY_TEST_CHILD", name)
        .env("MAESTRO_SECURITY_BRIDGE", &url)
        .env("MAESTRO_SECURITY_RUNNER", &url)
        .env("MAESTRO_SECURITY_BRIDGE_KEY", "test-only")
        .env("MAESTRO_SECURITY_MODEL", "primary")
        .status();
    stop.store(true, Ordering::Relaxed);
    assert!(server.join().is_ok(), "fixture server failed");
    assert!(status?.success(), "isolated test failed: {name}");
    Ok(false)
}

/// Accept requests until the child process finishes.
fn serve(listener: &TcpListener, stop: &AtomicBool, handler: Handler) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                assert!(respond(&mut stream, handler).is_ok(), "fixture HTTP error");
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => assert!(stop.load(Ordering::Relaxed), "accept: {error}"),
        }
    }
}

/// Read one content-length-framed request and send a connection-close response.
fn respond(stream: &mut TcpStream, handler: Handler) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut bytes = Vec::new();
    let mut buffer = [0; 4096];
    let end = loop {
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(index) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            break index + 4;
        }
    };
    let header = String::from_utf8_lossy(&bytes[..end]).into_owned();
    let length = header
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())?
        })
        .unwrap_or(0);
    while bytes.len() < end + length {
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    let path = header.split_whitespace().nth(1).unwrap_or("");
    let body = serde_json::from_slice(&bytes[end..]).unwrap_or(Value::Null);
    let (status, response) = handler(path, &body);
    write!(
        stream,
        "HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
        response.len()
    )
}

/// A bridge completion carrying the supplied model answer.
pub fn completion(answer: &str) -> (u16, String) {
    (
        200,
        json!({"choices": [{"message": {"content": answer}}]}).to_string(),
    )
}

/// A successful runner execution, without running any host command.
pub fn execution(output: &str, code: i32) -> (u16, String) {
    (
        200,
        format!(
            "{}\n{}\n",
            json!({"type":"out", "data":output}),
            json!({"type":"exit", "code":code})
        ),
    )
}

/// A catalog containing both models used by benchmark tests.
pub fn models() -> (u16, String) {
    (
        200,
        json!({"data":[{"id":"primary"},{"id":"secondary"}]}).to_string(),
    )
}
