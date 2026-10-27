//! Loopback services and process-local configuration for capability tests.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

/// Run the named test in a child with isolated state and loopback services.
pub fn isolated(name: &str, replies: &[&str], check: impl FnOnce(&str)) {
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
    let worker = std::thread::spawn(move || serve(&listener, &worker_stop, &replies));
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture"])
        .env("CAPABILITY_TEST_BASE", &base)
        .env("MAESTRO_SECURITY_BRIDGE", &base)
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
fn serve(listener: &TcpListener, stop: &AtomicBool, replies: &[String]) {
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut chats = 0;
    let mut observed = "no page or probe has been requested";
    while !stop.load(Ordering::Relaxed) && Instant::now() < deadline {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let request = read_request(&mut stream);
                let body = if request.starts_with("GET /page ") {
                    observed = "Login";
                    "<title>Login</title><a href='/login'>Sign in</a><form action='/auth' method='post'><input name='user'></form>".to_string()
                } else if request.starts_with("POST /chat/completions ") {
                    assert!(
                        request.contains(observed),
                        "captured data must reach the model"
                    );
                    assert!(
                        request.contains("fixture-model"),
                        "resolved model must reach the bridge"
                    );
                    let reply = &replies[chats];
                    chats += 1;
                    serde_json::json!({"choices":[{"message":{"content":reply}}]}).to_string()
                } else if request.starts_with("POST /exec ") {
                    assert!(
                        request.contains("example.test"),
                        "requested target must reach the sandbox"
                    );
                    observed = "22/tcp open ssh";
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
}

/// Read headers and the complete declared body before answering.
fn read_request(stream: &mut TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut bytes = Vec::new();
    let mut buffer = [0; 4096];
    loop {
        let size = stream.read(&mut buffer).unwrap();
        assert_ne!(size, 0, "incomplete HTTP request");
        bytes.extend_from_slice(&buffer[..size]);
        if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&bytes[..end]);
            let length = headers
                .lines()
                .find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    key.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            if bytes.len() >= end + 4 + length {
                return String::from_utf8(bytes).unwrap();
            }
        }
    }
}
