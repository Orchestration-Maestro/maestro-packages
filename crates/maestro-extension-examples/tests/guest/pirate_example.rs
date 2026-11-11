//! The shipped prompt-mode toggle loaded into the production engine.
//!
//! Notification severities are the host's own spellings, so the source's `info` is `Info`.
use super::support::{Recorder, before_agent_start, component, harness, load, local};
use maestro_extensions_wasmtime::test_support::{Harness, Loaded};
use serde_json::{Value, json};
use std::rc::Rc;

/// The variable that names the built toggle component.
const COMPONENT: &str = "MAESTRO_PIRATE_COMPONENT";

/// The recorded registration and observation rows these tests consume.
fn corpus() -> Value {
    serde_json::from_str(include_str!("pirate_expected.json")).unwrap()
}

/// One loaded instance of the component with the recording UI its notifications reach.
struct Instance {
    /// The engine the instance lives in.
    harness: Harness,
    /// The runner over the loaded component.
    loaded: Loaded,
    /// The notifications the component showed.
    ui: Rc<Recorder>,
}

impl Instance {
    /// Loads one independent instance.
    async fn new() -> Self {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let ui = Recorder::install(&loaded.runner);
        Self {
            harness,
            loaded,
            ui,
        }
    }

    /// Runs the command with `input` and returns what the UI recorded and the command's error.
    async fn command(&self, input: &Value) -> (Value, Option<String>) {
        let command = self.loaded.runner.get_command("pirate").unwrap().command;
        match input["rejection"].as_str() {
            Some(rejection) => self.ui.reject("notify", rejection),
            None => self.ui.disarm(),
        }
        let outcome = (command.handler)(
            input["args"].as_str().unwrap().into(),
            self.loaded.runner.create_command_context(),
        )
        .await;
        (
            json!(
                self.ui
                    .take()
                    .iter()
                    .map(|call| json!({"message": call[1][0], "kind": call[1][1]}))
                    .collect::<Vec<_>>()
            ),
            outcome.err().map(|error| error.to_string()),
        )
    }
}

/// Replays one recorded row against the instance it selects.
async fn run(instances: &[Instance], row: &Value) {
    let instance = &instances[usize::try_from(row["instance"].as_u64().unwrap()).unwrap()];
    let name = &row["name"];
    let expected = &row["expected"];
    if row["operation"] == "command" {
        let (seen, error) = instance.command(&row["input"]).await;
        assert_eq!(seen, expected["notifications"], "{name}: notifications");
        assert_eq!(
            error.as_deref(),
            expected["error"].as_str(),
            "{name}: error"
        );
    } else {
        let answer = before_agent_start(&instance.loaded.runner, &row["input"]).await;
        assert_eq!(answer, expected["result"], "{name}");
    }
}

/// Builds `count` independent instances.
async fn instances(count: usize) -> Vec<Instance> {
    let mut built = Vec::new();
    for _ in 0..count {
        built.push(Instance::new().await);
    }
    built
}

/// Replays the rows of one group over `count` instances.
fn replay(group: &str, count: usize) {
    local(async {
        let corpus = corpus();
        let rows: Vec<&Value> = corpus["rows"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["group"] == group)
            .collect();
        assert!(!rows.is_empty(), "recorded rows of {group}");
        let hosts = instances(count).await;
        for row in rows {
            run(&hosts, row).await;
        }
    });
}

/// The shipped factory registers exactly one command and one prompt handler.
#[test]
fn pirate_registers_command_and_prompt_handler() {
    local(async {
        let instance = Instance::new().await;
        let runner = &instance.loaded.runner;
        let recorded = &corpus()["metadata"]["command"];
        let commands = runner.get_registered_commands();
        assert_eq!(commands.len(), 1);
        let command = &commands[0].command;
        assert_eq!(json!(command.name), recorded["name"]);
        assert_eq!(json!(command.description), recorded["description"]);
        assert_eq!(
            command.get_argument_completions.is_some(),
            recorded["completions"]
        );
        assert!(runner.has_handlers("before_agent_start"));
        drop(instance.harness);
    });
}

/// Every recorded toggling command and prompt observation of one instance.
#[test]
fn pirate_toggle_and_prompts_match_recorded_inputs() {
    replay("toggle", 1);
}

/// Two factory instances keep the toggle state they each own.
#[test]
fn pirate_factories_keep_separate_state() {
    local(async {
        let hosts = instances(2).await;
        let enabling = json!({"args": "", "rejection": null});
        let (_, error) = hosts[0].command(&enabling).await;
        assert_eq!(error, None);
        let corpus = corpus();
        for row in corpus["rows"].as_array().unwrap() {
            if row["group"] == "instances" {
                run(&hosts, row).await;
            }
        }
    });
}

/// A refused notification reports the host error and leaves the toggle on.
#[test]
fn pirate_notify_failure_keeps_toggle() {
    replay("rejection", 3);
}

/// The built single-copy example replays every recorded operation.
#[test]
fn pirate_component_runs_recorded_transitions() {
    local(async {
        let corpus = corpus();
        let hosts = instances(3).await;
        let path = component(COMPONENT);
        assert!(std::path::Path::new(&path).is_file());
        for row in corpus["rows"].as_array().unwrap() {
            run(&hosts, row).await;
        }
    });
}
