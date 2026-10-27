//! Loopback fixtures for public-client and isolated capability tests.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use maestro_security::Client;

/// Fallible test outcome, including fixture IO failures.
pub type TestResult = Result<(), Box<dyn std::error::Error>>;
/// The captured HTTP request returned by a completed bridge fixture.
pub type Capture = JoinHandle<std::io::Result<String>>;

/// Serve one response on an ephemeral port without contacting a real model.
///
/// # Errors
/// Returns an error if the loopback listener cannot be configured.
pub fn bridge(status: u16, body: &str) -> Result<(Client, Capture), Box<dyn std::error::Error>> {
    let (url, handle) = bridge_url(status, body)?;
    Ok((
        Client::local().with_base_url(url).with_key("fixture-key"),
        handle,
    ))
}

/// Serve an endpoint usable through environment-based model configuration.
///
/// # Errors
/// Returns an error if the loopback listener cannot be configured.
pub fn bridge_url(
    status: u16,
    body: &str,
) -> Result<(String, Capture), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let url = format!("http://{}/v1", listener.local_addr()?);
    let body = body.to_owned();
    let handle = thread::spawn(move || {
        let start = Instant::now();
        let mut socket = loop {
            if start.elapsed() > Duration::from_secs(10) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "no request",
                ));
            }
            match listener.accept() {
                Ok((socket, _)) => break socket,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => return Err(error),
            }
        };
        socket.set_read_timeout(Some(Duration::from_secs(10)))?;
        socket.set_write_timeout(Some(Duration::from_secs(10)))?;
        let request = read_request(&mut socket)?;
        write!(
            socket,
            "HTTP/1.1 {status} Reply\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        )?;
        Ok(request)
    });
    Ok((url, handle))
}

/// Read a header value without depending on the sender's header-name casing.
#[must_use]
pub fn header_value<'a>(request: &'a str, name: &str) -> Option<&'a str> {
    request
        .lines()
        .take_while(|line| !line.is_empty())
        .filter_map(|line| line.split_once(':'))
        .find_map(|(key, value)| key.eq_ignore_ascii_case(name).then_some(value.trim()))
}

/// Read the framed request before replying, including a POST body.
fn read_request(socket: &mut TcpStream) -> std::io::Result<String> {
    let mut reader = BufReader::new(socket);
    let mut head = String::new();
    let mut length = 0;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "request head",
            ));
        }
        head.push_str(&line);
        if line == "\r\n" {
            break;
        }
        if let Some(value) = header_value(&line, "Content-Length") {
            length = value
                .parse()
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    head.push_str(&String::from_utf8_lossy(&body));
    Ok(head)
}

/// Valid header-name casing must not cause the fixture to discard a body.
#[test]
fn request_framing_accepts_header_name_casing() -> TestResult {
    for (auth, kind, length) in [
        ("authorization", "content-type", "content-length"),
        ("Authorization", "Content-Type", "Content-Length"),
        ("AUTHORIZATION", "CONTENT-TYPE", "CONTENT-LENGTH"),
        ("aUtHoRiZaTiOn", "cOnTeNt-TyPe", "cOnTeNt-LeNgTh"),
    ] {
        let (url, server) = bridge_url(200, "")?;
        let address = url.trim_start_matches("http://").trim_end_matches("/v1");
        let mut socket = TcpStream::connect(address)?;
        socket.set_read_timeout(Some(Duration::from_secs(10)))?;
        let request = format!(
            "POST /v1 HTTP/1.1\r\n{auth}: Bearer fixture-key\r\n{kind}: application/json\r\n{length}: 4\r\n\r\nbody"
        );
        socket.write_all(request.as_bytes())?;
        socket.read_to_string(&mut String::new())?;
        let captured = captured(server)?;
        assert_eq!(captured, request, "{length}");
        assert_eq!(
            header_value(&captured, "Authorization"),
            Some("Bearer fixture-key")
        );
        assert_eq!(
            header_value(&captured, "Content-Type"),
            Some("application/json")
        );
        assert_eq!(header_value(&captured, "Content-Length"), Some("4"));
    }
    Ok(())
}

/// Join the fixture and surface server-side failures to the test runner.
///
/// # Errors
/// Returns an error if the fixture thread panics or fails to serve the request.
pub fn captured(handle: Capture) -> Result<String, Box<dyn std::error::Error>> {
    Ok(handle
        .join()
        .map_err(|_| "bridge fixture thread failed")??)
}

/// Run the named test in a child with isolated state and loopback services.
///
/// # Panics
/// Panics if fixture setup, cleanup, or a child or server assertion fails.
pub fn isolated(name: &str, model: &str, replies: &[&str], check: impl FnOnce(&str)) {
    if let Ok(base) = std::env::var("CAPABILITY_TEST_BASE") {
        check(&base);
        return;
    }
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let home = std::env::temp_dir().join(format!("capability-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&home).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = Arc::clone(&stop);
    let replies: Vec<String> = replies.iter().map(ToString::to_string).collect();
    let model = model.to_string();
    let worker = std::thread::spawn(move || serve(&listener, &worker_stop, &model, &replies));
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture"])
        .env("CAPABILITY_TEST_BASE", &base)
        .env("MAESTRO_SECURITY_BRIDGE", &base)
        .env("MAESTRO_SECURITY_BRIDGE_KEY", "fixture-key")
        .env("MAESTRO_SECURITY_RUNNER", &base)
        .env("MAESTRO_SECURITY_HOME", &home)
        .output()
        .unwrap();
    stop.store(true, Ordering::Relaxed);
    worker.join().unwrap();
    std::fs::remove_dir_all(home).unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Serve only the deterministic page, bridge and sandbox responses.
fn serve(listener: &TcpListener, stop: &AtomicBool, model: &str, replies: &[String]) {
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut chats = 0;
    let mut observed = None;
    while !stop.load(Ordering::Relaxed) && Instant::now() < deadline {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let request = read_request(&mut stream).unwrap();
                let body = if request.starts_with("GET /page ") {
                    observed = Some("Login");
                    "<title>Login</title><a href='/login'>Sign in</a><form action='/auth' method='post'><input name='user'></form>".to_string()
                } else if request.starts_with("POST /chat/completions ") {
                    assert!(
                        request.contains(
                            observed
                                .take()
                                .expect("each chat requires a fresh page or probe")
                        ),
                        "captured data must reach the model"
                    );
                    let (_, body) = request.split_once("\r\n\r\n").unwrap();
                    let body: serde_json::Value = serde_json::from_str(body).unwrap();
                    assert_eq!(body["model"], model, "resolved model must reach the bridge");
                    let reply = &replies[chats];
                    chats += 1;
                    serde_json::json!({"choices":[{"message":{"content":reply}}]}).to_string()
                } else if request.starts_with("POST /exec ") {
                    assert!(
                        request.contains("example.test"),
                        "requested target must reach the sandbox"
                    );
                    observed = Some("22/tcp open ssh");
                    "{\"type\":\"out\",\"data\":\"22/tcp open ssh\"}\n{\"type\":\"exit\",\"code\":0}\n".to_string()
                } else {
                    assert!(request.starts_with("GET /models "), "{request}");
                    "{\"data\":[{\"id\":\"fixture-model\"}]}".to_string()
                };
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(error) => panic!("fixture accept: {error}"),
        }
    }
    assert_eq!(
        chats,
        replies.len(),
        "every scripted reply must be consumed"
    );
}
