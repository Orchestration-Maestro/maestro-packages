//! Baseline context behaviour against ephemeral loopback services.
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;

use super::CapabilityCtx;
use crate::pipeline::Config;

/// Keep recording requests until the test explicitly shuts down the listener.
struct Service {
    /// Signal that the caller has completed its HTTP operations.
    shutdown: Sender<()>,
    /// The listener thread and its captured requests.
    thread: JoinHandle<Vec<String>>,
}

impl Service {
    /// Stop accepting requests and return everything received.
    fn finish(self) -> Vec<String> {
        self.shutdown.send(()).unwrap();
        self.thread.join().unwrap()
    }
}

/// Serve canned replies, then reject and record any unexpected requests.
fn service(replies: Vec<(u16, &'static str)>) -> (String, Service) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (shutdown, stopped) = channel();
    let thread = std::thread::spawn(move || {
        let mut requests = Vec::new();
        let mut replies = replies.into_iter();
        while let Some(mut stream) = accept(&listener, &stopped) {
            let (status, body) = replies.next().unwrap_or((500, "unexpected request"));
            let request = read_request(&mut stream);
            requests.push(request);
            write!(
                stream,
                "HTTP/1.1 {status} Reply\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
        requests
    });
    (url, Service { shutdown, thread })
}

/// Accept a connection until explicit shutdown or the fixture deadline.
fn accept(listener: &TcpListener, stopped: &Receiver<()>) -> Option<TcpStream> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        if stopped.try_recv() != Err(std::sync::mpsc::TryRecvError::Empty) {
            return None;
        }
        match listener.accept() {
            Ok((stream, _)) => return Some(stream),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(std::time::Instant::now() < deadline, "request not received");
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Err(error) => panic!("accept: {error}"),
        }
    }
}

/// Read the complete HTTP body even when TCP splits it across reads.
fn read_request(stream: &mut TcpStream) -> String {
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    let mut request = Vec::new();
    let mut buffer = [0; 4096];
    loop {
        let count = stream.read(&mut buffer).unwrap();
        assert!(count > 0, "incomplete request");
        request.extend_from_slice(&buffer[..count]);
        let text = String::from_utf8_lossy(&request);
        let Some((head, content)) = text.split_once("\r\n\r\n") else {
            continue;
        };
        let length = head
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().unwrap())
            })
            .unwrap_or(0);
        if content.len() >= length {
            return String::from_utf8(request).unwrap();
        }
    }
}

/// Configuration with explicit test-only endpoints, never the local router.
fn config(bridge: String) -> Config {
    Config {
        bridge,
        bridge_key: "fixture-key".into(),
        runner: "http://127.0.0.1:1".into(),
        default_model: "preferred".into(),
        max_hypotheses: 1,
    }
}

/// Baseline: preferred discovery is resolved once and forwarded with chat settings.
#[test]
fn baseline_discovery_and_chat() {
    let (bridge, server) = service(vec![
        (200, r#"{"data":[{"id":"first"},{"id":"preferred"}]}"#),
        (200, r#"{"choices":[{"message":{"content":"answer"}}]}"#),
    ]);
    let ctx = CapabilityCtx::build(&config(bridge), None).unwrap();
    assert_eq!(ctx.model, "preferred");
    assert_eq!(
        ctx.chat("verify access", "observed response", 123).unwrap(),
        "answer"
    );
    let requests = server.finish();
    assert!(requests[0].starts_with("GET /models "));
    assert!(requests[1].starts_with("POST /chat/completions "));
    assert!(
        requests[1]
            .to_lowercase()
            .contains("authorization: bearer fixture-key\r\n")
    );
    let body: serde_json::Value =
        serde_json::from_str(requests[1].split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(body["model"], "preferred");
    assert_eq!(body["max_tokens"], 123);
    assert_eq!(body["messages"][0]["role"], "system");
    let system = body["messages"][0]["content"].as_str().unwrap();
    assert!(system.starts_with(crate::methodology::SYSTEM_PROMPT));
    assert!(system.ends_with("CAPABILITY ROLE:\nverify access"));
    assert_eq!(body["messages"][1]["role"], "user");
    assert_eq!(body["messages"][1]["content"], "observed response");
}

/// Baseline: unavailable preferred models fall back to the first served model.
#[test]
fn baseline_model_fallback() {
    let (bridge, server) = service(vec![(200, r#"{"data":[{"id":"first"},{"id":"second"}]}"#)]);
    let ctx = CapabilityCtx::build(&config(bridge), None).unwrap();
    assert_eq!(ctx.model, "first");
    assert_eq!(server.finish().len(), 1);
}

/// Baseline: overrides skip discovery; chat failures remain failures.
#[test]
fn baseline_override_and_chat_errors() {
    for (status, reply, cause) in [
        (401, "denied", "401"),
        (200, r#"{"choices":[]}"#, "no message content"),
        (200, "{bad", "json"),
    ] {
        let (bridge, server) = service(vec![(status, reply)]);
        let ctx = CapabilityCtx::build(&config(bridge), Some("override")).unwrap();
        assert_eq!(ctx.model, "override");
        let error = ctx.chat("role", "request", 1).unwrap_err();
        assert!(error.contains(cause), "{error}");
        let requests = server.finish();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].starts_with("POST /chat/completions "));
    }
}

/// Baseline: discovery failures stop context construction.
#[test]
fn baseline_discovery_errors() {
    for (status, reply, cause) in [
        (200, r#"{"data":[]}"#, "bridge /models returned no models"),
        (401, "denied", "bridge http 401 on /models"),
        (200, "{bad", "bridge json:"),
    ] {
        let (bridge, server) = service(vec![(status, reply)]);
        let error = CapabilityCtx::build(&config(bridge), None).err().unwrap();
        assert!(error.contains(cause), "{error}");
        assert_eq!(server.finish().len(), 1);
    }
}

/// Baseline: the sandbox uses its configured endpoint rather than its default.
#[test]
fn baseline_sandbox_endpoint() {
    let (runner, server) = service(vec![(200, "{}")]);
    let mut config = config("http://127.0.0.1:1".into());
    config.runner = runner;
    let ctx = CapabilityCtx::build(&config, Some("override")).unwrap();
    ctx.sandbox.health().unwrap();
    assert!(server.finish()[0].starts_with("GET /health "));
}
