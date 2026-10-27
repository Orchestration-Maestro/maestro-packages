//! Behavioral checks over the public MCP stdio transport.
#![cfg(test)]
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// A server process with bounded response waits and cleanup on test failure.
struct Client {
    /// Server killed when the fixture drops.
    child: Child,
    /// Newline-delimited JSON-RPC requests.
    input: Option<ChildStdin>,
    /// Responses read independently so a broken server cannot hang the test.
    output: Receiver<Value>,
    /// Isolated persistent state.
    home: std::path::PathBuf,
}

impl Client {
    /// Start the shipped executable and complete the MCP handshake.
    fn start() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let home = std::env::temp_dir().join(format!(
            "security-mcp-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut child = Command::new(env!("CARGO_BIN_EXE_maestro-security-mcp"))
            .env("MAESTRO_SECURITY_HOME", &home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("MCP server");
        let input = child.stdin.take().expect("stdin");
        let output = child.stdout.take().expect("stdout");
        let (sender, receiver) = channel();
        std::thread::spawn(move || forward_lines(output, &sender));
        let mut client = Self {
            child,
            input: Some(input),
            output: receiver,
            home,
        };
        let result = client.request(
            1,
            "initialize",
            &json!({
                "protocolVersion":"2025-11-25", "capabilities":{},
                "clientInfo":{"name":"behavior-test","version":"1"}
            }),
        );
        assert_eq!(result["serverInfo"]["name"], "maestro-security");
        assert!(result["capabilities"]["tools"].is_object());
        client.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        client
    }

    /// Send a message to the server.
    fn send(&mut self, message: &Value) {
        let input = self.input.as_mut().expect("open transport");
        writeln!(input, "{message}").expect("write request");
        input.flush().expect("flush request");
    }

    /// Require a protocol-success response with the caller's request id.
    fn request(&mut self, id: u32, method: &str, params: &Value) -> Value {
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
        let response = self
            .output
            .recv_timeout(Duration::from_secs(10))
            .expect("server response");
        assert_eq!(response["id"], id);
        assert!(
            response.get("error").is_none(),
            "protocol error: {response}"
        );
        response["result"].clone()
    }
}

/// Forward transport lines without borrowing the client across threads.
fn forward_lines(output: std::process::ChildStdout, sender: &std::sync::mpsc::Sender<Value>) {
    for line in BufReader::new(output).lines() {
        let value = serde_json::from_str(&line.expect("server line")).expect("JSON-RPC response");
        if sender.send(value).is_err() {
            break;
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.input.take();
        let deadline = Instant::now() + Duration::from_secs(2);
        while self.child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.home);
    }
}

/// Baseline: discoverable tools accept calls; failures stay readable tool results.
#[test]
fn baseline_mcp_discovers_tools_and_separates_tool_errors_from_protocol_errors() {
    let mut client = Client::start();
    let listed = client.request(2, "tools/list", &json!({}));
    let tools = listed["tools"].as_array().expect("tools");
    let mut names: Vec<_> = tools
        .iter()
        .map(|tool| tool["name"].as_str().expect("name"))
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "security_assess",
            "security_disclose",
            "security_doctor",
            "security_evidence",
            "security_evidence_page",
            "security_report_page",
            "security_route",
            "security_validate",
            "security_workspace"
        ]
    );
    for tool in tools {
        assert!(tool["inputSchema"].is_object());
    }
    check_tool_calls(&mut client);
}

/// Check error results and a successful state-reading call over the same session.
fn check_tool_calls(client: &mut Client) {
    for name in [
        "unknown",
        "security_assess",
        "security_evidence",
        "security_workspace",
        "security_route",
        "security_report_page",
        "security_evidence_page",
        "security_validate",
        "security_disclose",
    ] {
        let result = client.request(3, "tools/call", &json!({"name":name,"arguments":{}}));
        assert_eq!(result["isError"], true, "{name}: {result}");
        assert_eq!(result["content"][0]["type"], "text");
        assert!(
            !result["content"][0]["text"]
                .as_str()
                .expect("error text")
                .is_empty()
        );
    }
    let result = client.request(
        4,
        "tools/call",
        &json!({"name":"security_workspace","arguments":{"action":"list"}}),
    );
    assert_ne!(result["isError"], true);
    let board: Value =
        serde_json::from_str(result["content"][0]["text"].as_str().expect("board text"))
            .expect("board JSON");
    assert_eq!(board["action"], "list");
    let result = client.request(5, "tools/call", &json!({"name":"security_evidence"}));
    assert_eq!(
        result["isError"], true,
        "omitted arguments are a tool error"
    );
}

/// Closing the initialized transport must not panic or return a server error.
#[test]
fn mcp_stdio_shuts_down_successfully_on_eof() {
    let mut client = Client::start();
    client.input.take();
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = client.child.try_wait().expect("server status") {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "server failed to close after EOF"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(status.success(), "MCP shutdown failed: {status}");
}
