//! The shipped dynamic-tools example loaded into the production engine.
//!
//! A refused tool registration cannot be injected into the engine's registry, so the two
//! recorded rows that need it are not replayed.
use super::lane_a::{Ui, event, failures};
use super::support::{harness, load, local, result_value};
use maestro_extensions_wasmtime::test_support::{
    Harness, Loaded,
    maestro_extensions::{
        AgentToolUpdateCallback, Cancellation, Local, SessionStartEvent, wrap_registered_tool,
    },
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// The variable that names the built dynamic-tools component.
const COMPONENT: &str = "MAESTRO_DYNAMIC_TOOLS_COMPONENT";
/// The failure every refused notification carries.
const FAILURE: &str = "host refused Ω\ncontrolled";

/// The recorded rows these tests consume.
fn corpus() -> Value {
    serde_json::from_str(include_str!("dynamic_tools_expected.json")).unwrap()
}

/// The recorded rows of `group` that the engine can reproduce.
fn rows(group: &str) -> Vec<Value> {
    let rows: Vec<Value> = corpus()["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["group"] == group)
        .filter(|row| {
            !row["steps"]
                .as_array()
                .unwrap()
                .iter()
                .any(|step| matches!(step["reject"].as_str(), Some("registerTool" | "ui")))
        })
        .cloned()
        .collect();
    assert!(!rows.is_empty(), "recorded rows of {group}");
    rows
}

/// One loaded instance with the recording UI its notifications reach.
struct Instance {
    /// The engine the instance lives in.
    harness: Harness,
    /// The runner over the loaded component.
    loaded: Loaded,
    /// The notifications the component showed.
    ui: Rc<Ui>,
    /// The callback failures the runner reported.
    failures: Rc<RefCell<Vec<String>>>,
    /// The names of the tools the last step had already seen.
    known: RefCell<Vec<String>>,
}

impl Instance {
    /// Loads one independent instance.
    async fn new() -> Self {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let ui = Ui::install(&loaded.runner);
        let failures = failures(&loaded.runner);
        Self {
            harness,
            loaded,
            ui,
            failures,
            known: RefCell::new(Vec::new()),
        }
    }

    /// The registered tools' names, in registration order.
    fn visible(&self) -> Vec<String> {
        self.loaded
            .runner
            .get_all_registered_tools()
            .iter()
            .map(|tool| tool.metadata().definition.name)
            .collect()
    }

    /// The effects since the last step: each new tool's declaration, then each notification.
    fn effects(&self) -> Vec<Value> {
        let known = self.known.borrow().clone();
        let mut effects = Vec::new();
        for tool in self.loaded.runner.get_all_registered_tools() {
            let metadata = tool.metadata();
            if known.contains(&metadata.definition.name) {
                continue;
            }
            effects.push(json!({"registerTool": {
                "name": metadata.definition.name,
                "label": metadata.label,
                "description": metadata.definition.description,
                "promptSnippet": metadata.prompt_snippet,
                "promptGuidelines": metadata.prompt_guidelines,
                "parameters": serde_json::to_value(&metadata.definition.parameters).unwrap(),
            }}));
        }
        for request in self.ui.take() {
            let kind = request["operands"][1].as_str().unwrap().to_lowercase();
            effects.push(json!({"notify": {"message": request["operands"][0], "kind": kind}}));
        }
        *self.known.borrow_mut() = self.visible();
        effects
    }

    /// Runs one recorded step; returns the tool result and the error it ended with.
    async fn run(&self, step: &Value) -> (Value, Option<String>) {
        let runner = &self.loaded.runner;
        if step["reject"] == "notify" {
            self.ui.reject("notify", FAILURE);
        }
        match step["op"].as_str().unwrap() {
            "command" => {
                let command = runner.get_command("add-echo-tool").unwrap().command;
                let outcome = (command.handler)(
                    step["args"].as_str().unwrap().into(),
                    runner.create_command_context(),
                )
                .await;
                (Value::Null, outcome.err().map(|error| error.to_string()))
            }
            "session_start" => {
                let mut start: SessionStartEvent =
                    event(json!({"type": "session_start", "reason": "new"}));
                runner.emit(&mut start).await.unwrap();
                (Value::Null, self.failures.borrow_mut().pop())
            }
            _ => (self.execute(step).await, None),
        }
    }

    /// Executes the recorded tool call, with the cancellation and progress handles it names.
    async fn execute(&self, step: &Value) -> Value {
        let runner = &self.loaded.runner;
        let tools = runner.get_all_registered_tools();
        let name = step["tool"].as_str().unwrap();
        let registered = tools
            .iter()
            .find(|tool| tool.metadata().definition.name == name)
            .unwrap();
        let tool = wrap_registered_tool(registered, runner.clone());
        let signal = step["aborted"].as_bool().unwrap_or(false).then(|| {
            let signal = Cancellation::new();
            signal.abort();
            signal
        });
        let updates = Rc::new(RefCell::new(0_usize));
        let counted = updates.clone();
        let update: Option<AgentToolUpdateCallback<Value, Local>> = step["progress"]
            .as_bool()
            .unwrap_or(false)
            .then(|| Rc::new(move |_| *counted.borrow_mut() += 1) as _);
        let result = (tool.execute)("call".into(), step["params"].clone(), signal, update)
            .await
            .unwrap();
        assert_eq!(*updates.borrow(), 0, "the echo tool reports no progress");
        result_value(&result)
    }
}

/// Replays one recorded row against the instances it names.
async fn replay(row: &Value) {
    let count = usize::try_from(row["instances"].as_u64().unwrap()).unwrap();
    let mut instances = Vec::new();
    for _ in 0..count {
        instances.push(Instance::new().await);
    }
    let name = &row["name"];
    for (index, step) in row["steps"].as_array().unwrap().iter().enumerate() {
        let instance = &instances[usize::try_from(step["instance"].as_u64().unwrap_or(0)).unwrap()];
        let (result, error) = instance.run(step).await;
        let expected = &row["observations"][index];
        let at = format!("{name} step {index}");
        assert_eq!(
            Value::Array(instance.effects()),
            expected["effects"],
            "{at}: effects"
        );
        assert_eq!(
            json!(instance.visible()),
            expected["visible"],
            "{at}: visible"
        );
        assert_eq!(result, expected["result"], "{at}: result");
        assert_eq!(error.as_deref(), expected["error"].as_str(), "{at}: error");
    }
    drop(instances);
}

/// Replays every reproducible row of `group`.
fn replay_group(group: &str) {
    local(async {
        for row in rows(group) {
            replay(&row).await;
        }
    });
}

/// The example registers its session hook and its command, and no tool until one is requested.
#[test]
fn dynamic_tools_registers_deferred_tools_and_command() {
    local(async {
        let instance = Instance::new().await;
        let runner = &instance.loaded.runner;
        assert!(runner.has_handlers("session_start"));
        let commands = runner.get_registered_commands();
        assert_eq!(commands.len(), 1);
        let command = &commands[0].command;
        let recorded = &corpus()["rows"][0]["registrations"][0];
        assert_eq!(
            recorded,
            &json!([
                {"event": "session_start"},
                {"command": {
                    "name": command.name,
                    "description": command.description,
                    "completions": command.get_argument_completions.is_some(),
                }}
            ])
        );
        assert!(instance.visible().is_empty(), "no tool before a request");
        drop(instance.harness);
    });
}

/// Names register only when they are lowercase letters, digits and underscores after trimming.
#[test]
fn dynamic_tools_names_follow_argument_rules() {
    replay_group("names");
}

/// The session hook and the command register live tools that run when called.
#[test]
fn dynamic_tools_session_and_command_register_live_tools() {
    replay_group("lifecycle");
}

/// A name registered twice keeps its first definition, per factory instance.
#[test]
fn dynamic_tools_duplicate_names_keep_first_registration() {
    replay_group("state");
}

/// Every echoed message keeps its text exactly, and unused handles change nothing.
#[test]
fn dynamic_tools_echo_preserves_messages() {
    replay_group("echo");
}

/// A refused notification keeps the registration that came before it.
#[test]
fn dynamic_tools_host_failures_keep_prior_state() {
    replay_group("errors");
}
