//! Loopback runner fixtures isolated in a child test process.
#![cfg(test)]
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;
use std::time::{Duration, Instant};

/// Run this test in a child with a scripted runner, returning true in the child.
pub fn runner(test: &str, replies: Vec<(u16, String)>) -> bool {
    if std::env::var("SECURITY_TEST_CHILD").as_deref() == Ok(test) {
        return true;
    }
    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback runner");
    listener.set_nonblocking(true).expect("nonblocking");
    let url = format!("http://{}", listener.local_addr().expect("address"));
    let server = std::thread::spawn(move || {
        for (status, body) in replies {
            let mut stream = accept(&listener);
            read_request(&stream);
            write!(stream, "HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).expect("response");
        }
    });
    let output = Command::new(std::env::current_exe().expect("test executable"))
        .args(["--exact", test, "--nocapture"])
        .env("SECURITY_TEST_CHILD", test)
        .env("MAESTRO_SECURITY_RUNNER", url)
        .env("MAESTRO_SECURITY_BRIDGE", "http://127.0.0.1:1")
        .output()
        .expect("child test");
    server.join().expect("runner fixture");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    false
}

/// Wait for one request without hanging when a regression omits it.
fn accept(listener: &TcpListener) -> TcpStream {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match listener.accept() {
            Ok((stream, _)) => return stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "runner request timed out");
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("accept: {error}"),
        }
    }
}

/// Drain the request body before closing the connection.
fn read_request(stream: &TcpStream) {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("read timeout");
    let mut reader = BufReader::new(stream);
    let mut length = 0;
    loop {
        let mut line = String::new();
        assert!(reader.read_line(&mut line).expect("request header") > 0);
        if line == "\r\n" {
            break;
        }
        if let Some((key, value)) = line.split_once(':')
            && key.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().expect("content length");
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).expect("request body");
}

/// Encode the runner's output stream for one successful command.
pub fn output(text: &str) -> (u16, String) {
    (
        200,
        format!(
            "{}\n{}\n",
            serde_json::json!({"type":"out", "data":text}),
            serde_json::json!({"type":"exit", "code":0})
        ),
    )
}
