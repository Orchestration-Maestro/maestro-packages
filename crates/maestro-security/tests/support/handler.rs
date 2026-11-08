//! Process-isolated HTTP fixtures for capability behaviour tests.

use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use maestro_test_directory::Scratch;
use maestro_test_extension_host::read_request;
use maestro_test_process::TestProcess;
use serde_json::{Value, json};

/// A fixture response selected from the request path and JSON body.
pub type Handler = fn(&str, &Value) -> (u16, String);

/// Run the named test in a fresh process with fixture-only endpoints.
/// Returns true in the child, where the caller exercises the capability.
pub fn isolated(name: &str, handler: Handler) -> Result<bool, Box<dyn std::error::Error>> {
    if std::env::var("SECURITY_TEST_CHILD").as_deref() == Ok(name) {
        return Ok(true);
    }
    let child = TestProcess::named(name)?;
    let home = Scratch::new()?;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let url = format!("http://{}", listener.local_addr()?);
    listener.set_nonblocking(true)?;
    let stop = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&stop);
    let server = thread::spawn(move || serve(&listener, &signal, handler));
    let status = child
        .nocapture()
        .env("SECURITY_TEST_CHILD", name)
        .env("MAESTRO_SECURITY_HOME", home.path())
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
                assert!(
                    stream.set_nonblocking(false).is_ok(),
                    "fixture socket must use blocking reads"
                );
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
    let request = read_request(&*stream)?;
    let path = request.head.split_whitespace().nth(1).unwrap_or("");
    let body = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
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
