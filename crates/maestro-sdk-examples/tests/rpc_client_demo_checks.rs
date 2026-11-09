//! Checks of the interactive RPC client demo against Maestro's native test peers.

use crate::rpc_client_demo::{Output, preview, report, run};
use crate::rpc_peer::{Peer, Scenario, events, next_of};
use crate::support::session_harness::on_task_set;
use maestro_app::{RpcClient, RpcClientError};
use serde_json::{Value, json};
use std::{cell::RefCell, io::Write, rc::Rc};
use tokio::sync::mpsc;

/// A session state with `model`.
fn state(model: Option<Value>) -> Value {
    let mut state = json!({"thinkingLevel": "high", "isStreaming": false, "isCompacting": false, "steeringMode": "all", "followUpMode": "all", "sessionId": "s", "autoCompactionEnabled": true, "messageCount": 0, "pendingMessageCount": 0});
    if let Some(model) = model {
        state["model"] = model;
    }
    state
}

/// The selected model of the illustration.
fn model() -> Value {
    json!({"provider": "anthropic", "id": "claude-sonnet-4-20250514", "name": "Sonnet", "api": "anthropic-messages", "baseUrl": "https://fixture.invalid", "reasoning": true, "input": ["text"], "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0}, "contextWindow": 1, "maxTokens": 1})
}

/// Output that announces each `You: ` prompt written to it.
struct Recorder {
    /// Everything written.
    text: Vec<u8>,
    /// Receives one signal per prompt.
    prompts: mpsc::UnboundedSender<()>,
}

impl Write for Recorder {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.text.extend_from_slice(bytes);
        if self.text.ends_with(b"You: ") {
            let _ = self.prompts.send(());
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// One flow under test, with its inputs and its recorded output.
struct Flow {
    /// The client the flow drives.
    client: Rc<RpcClient>,
    /// Feeds input lines.
    lines: mpsc::UnboundedSender<String>,
    /// Feeds interrupts.
    interrupt: mpsc::UnboundedSender<()>,
    /// Signals each written prompt.
    prompts: mpsc::UnboundedReceiver<()>,
    /// The recorded output.
    output: Rc<RefCell<Recorder>>,
    /// The running flow.
    run: tokio::task::JoinHandle<Result<(), RpcClientError>>,
}

impl Flow {
    /// Starts the flow on a client of `peer` whose session state is `state`.
    fn start(peer: &Peer, state: &Value) -> Self {
        let mut options = peer.options(Scenario::Session);
        options.env.insert("PEER_STATE".into(), state.to_string());
        let client = Rc::new(RpcClient::new(options));
        let (lines, input) = mpsc::unbounded_channel();
        let (interrupt, interrupts) = mpsc::unbounded_channel();
        let (signal, prompts) = mpsc::unbounded_channel();
        let output = Rc::new(RefCell::new(Recorder {
            text: Vec::new(),
            prompts: signal,
        }));
        let shared: Output = output.clone();
        let run = tokio::task::spawn_local(run(Rc::clone(&client), input, interrupts, shared));
        Self {
            client,
            lines,
            interrupt,
            prompts,
            output,
            run,
        }
    }

    /// Sends `line` once the next prompt is written.
    async fn answer(&mut self, line: &str) {
        self.prompts.recv().await.unwrap();
        self.lines.send(line.to_owned()).unwrap();
    }

    /// The flow's outcome once it ends.
    async fn finished(&mut self) -> Result<(), RpcClientError> {
        (&mut self.run).await.unwrap()
    }

    /// The recorded output.
    fn text(&self) -> String {
        String::from_utf8(self.output.borrow().text.clone()).unwrap()
    }

    /// The commands the child received, in order.
    fn commands(&self) -> Vec<Value> {
        let stderr = self.client.get_stderr();
        stderr
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    }
}

#[test]
fn illustration_streams_state_text_thinking_and_tool_output() {
    let peer = Peer::new();
    on_task_set(|_| async {
        for (model, selected) in [
            (Some(model()), "anthropic/claude-sonnet-4-20250514"),
            (None, "undefined/undefined"),
        ] {
            let mut flow = Flow::start(&peer, &state(model));
            flow.answer("hello").await;
            flow.answer("exit").await;
            flow.finished().await.unwrap();
            let header = format!(
                "Model: {selected}\nThinking: high\n\nInteractive RPC example. Type 'exit' to quit.\n\n"
            );
            let result = r#"{"content":[{"type":"text","text":"ok"}]}"#;
            let turn = format!("Hi there (thinking)\n[Tool: read]\n[Result: {result}...]\n\n");
            assert_eq!(flow.text(), format!("{header}You: {turn}\n\nYou: "));
        }
    });
}

#[test]
fn illustration_keeps_original_lines_and_exact_exit_whitespace() {
    let peer = Peer::new();
    on_task_set(|_| async {
        let mut flow = Flow::start(&peer, &state(None));
        flow.answer("  hello  ").await;
        flow.answer("\u{85}exit").await;
        flow.answer("\u{feff} exit\t").await;
        flow.finished().await.unwrap();
        let prompts: Vec<_> = flow
            .commands()
            .into_iter()
            .filter(|command| command["type"] == "prompt")
            .map(|command| command["message"].clone())
            .collect();
        assert_eq!(prompts, [json!("  hello  "), json!("\u{85}exit")]);
        assert!(matches!(
            flow.client.abort().await,
            Err(RpcClientError::NotStarted)
        ));
    });
}

#[test]
fn illustration_suppresses_input_while_a_turn_is_waiting() {
    let peer = Peer::new();
    on_task_set(|_| async {
        let mut flow = Flow::start(&peer, &state(None));
        let mut events = events(&flow.client);
        flow.answer("hold").await;
        next_of(&mut events, "agent_start").await;
        flow.lines.send("exit".into()).unwrap();
        flow.lines.send("ignored".into()).unwrap();
        flow.client.steer("finish", None).await.unwrap();
        flow.answer("exit").await;
        flow.finished().await.unwrap();
        let kinds: Vec<_> = flow
            .commands()
            .into_iter()
            .map(|command| command["type"].clone())
            .collect();
        assert_eq!(kinds, [json!("get_state"), json!("prompt"), json!("steer")]);
        assert!(flow.text().ends_with("You: \n\nYou: "));
    });
}

#[test]
fn illustration_interrupts_follow_the_waiting_state() {
    let peer = Peer::new();
    on_task_set(|_| async {
        let mut flow = Flow::start(&peer, &state(None));
        let mut events = events(&flow.client);
        flow.answer("hold").await;
        next_of(&mut events, "agent_start").await;
        flow.interrupt.send(()).unwrap();
        flow.prompts.recv().await.unwrap();
        let aborted = flow.text();
        assert!(aborted.ends_with("You: \n[Aborting...]\n\n\nYou: "));
        // An idle interrupt ends the flow without aborting; its termination request is
        // checked through the peer's control channel by the idle-interrupt check below.
        flow.interrupt.send(()).unwrap();
        flow.finished().await.unwrap();
        assert_eq!(flow.text(), aborted);
        let kinds: Vec<_> = flow
            .commands()
            .into_iter()
            .map(|command| command["type"].clone())
            .collect();
        assert_eq!(kinds, [json!("get_state"), json!("prompt"), json!("abort")]);
    });
}

#[test]
fn illustration_idle_interrupt_requests_termination_before_teardown() {
    let peer = Peer::new();
    let marker = peer.scratch.join("marker");
    let mut options = peer.options(Scenario::IdleInterrupt);
    options.env.insert("PEER_MARKER".into(), marker.clone());
    options
        .env
        .insert("PEER_STATE".into(), state(None).to_string());
    let (signal, mut prompts) = mpsc::unbounded_channel();
    let output: Output = Rc::new(RefCell::new(Recorder {
        text: Vec::new(),
        prompts: signal,
    }));
    let (_lines, input) = mpsc::unbounded_channel();
    let (interrupt, interrupts) = mpsc::unbounded_channel();
    let runtime = crate::support::io_runtime();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let interrupting = async {
            prompts.recv().await.unwrap();
            interrupt.send(()).unwrap();
        };
        let client = Rc::new(RpcClient::new(options));
        let (result, ()) = tokio::join!(run(client, input, interrupts, output), interrupting);
        result.unwrap();
    });
    // The task set is gone, as it is once the example's entry returns.
    let mut completed = std::io::BufReader::new(peer.control());
    let mut ended = String::new();
    std::io::BufRead::read_line(&mut completed, &mut ended).unwrap();
    assert_eq!(ended, "terminated\n");
    ended.clear();
    std::io::BufRead::read_line(&mut completed, &mut ended).unwrap();
    assert_eq!(ended, "input closed\n");
    let ended = std::fs::read_to_string(&marker).unwrap();
    assert_eq!(ended, "terminated");
}

#[test]
fn illustration_preview_keeps_the_boundary_scalar() {
    let quoted = |text: String| preview(&json!(text)).unwrap();
    assert_eq!(preview(&Value::Null).unwrap(), "null");
    for length in [199, 200] {
        let text = "a".repeat(length - 2);
        assert_eq!(quoted(text.clone()), format!("\"{text}\""));
    }
    assert_eq!(quoted("a".repeat(199)), format!("\"{}", "a".repeat(199)));
    let boundary = format!("{}\u{1f642}tail", "a".repeat(198));
    assert_eq!(quoted(boundary), format!("\"{}\u{1f642}", "a".repeat(198)));
}

#[test]
fn illustration_reports_top_level_error_without_extra_diagnostics() {
    let peer = Peer::new();
    on_task_set(|_| async {
        let mut options = peer.options(Scenario::Session);
        options.cli_path = Some(peer.scratch.join("missing"));
        let (_lines, input) = mpsc::unbounded_channel();
        let (_interrupt, interrupts) = mpsc::unbounded_channel();
        let (signal, _prompts) = mpsc::unbounded_channel();
        let output = Rc::new(RefCell::new(Recorder {
            text: Vec::new(),
            prompts: signal,
        }));
        let shared: Output = output.clone();
        let result = run(Rc::new(RpcClient::new(options)), input, interrupts, shared).await;
        assert!(output.borrow().text.is_empty());
        let mut stderr = Vec::new();
        report(result, &mut stderr);
        assert_eq!(
            String::from_utf8(stderr).unwrap(),
            "No such file or directory (os error 2)\n"
        );
        let mut stderr = Vec::new();
        report(Ok(()), &mut stderr);
        assert!(stderr.is_empty());
    });
}
