//! Baseline context behaviour against ephemeral loopback services.
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread::JoinHandle;

use super::CapabilityCtx;
use crate::pipeline::Config;

/// Serve a bounded sequence of HTTP replies and capture each request.
fn service(replies: Vec<(u16, &'static str)>) -> (String, JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let thread = std::thread::spawn(move || {
        let mut requests = Vec::new();
        for (status, body) in replies {
            let mut stream = accept(&listener);
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
    (url, thread)
}

/// Accept one loopback connection within the fixture deadline.
fn accept(listener: &TcpListener) -> TcpStream {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        match listener.accept() {
            Ok((stream, _)) => return stream,
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
    let requests = server.join().unwrap();
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
    assert_eq!(server.join().unwrap().len(), 1);
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
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].starts_with("POST /chat/completions "));
    }
}

/// Baseline: discovery failures stop context construction.
#[test]
fn baseline_discovery_errors() {
    for (status, reply) in [(200, r#"{"data":[]}"#), (401, "denied"), (200, "{bad")] {
        let (bridge, server) = service(vec![(status, reply)]);
        assert!(CapabilityCtx::build(&config(bridge), None).is_err());
        assert_eq!(server.join().unwrap().len(), 1);
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
    assert!(server.join().unwrap()[0].starts_with("GET /health "));
}
