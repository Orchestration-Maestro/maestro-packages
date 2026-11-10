//! The shipped hidden-thinking-label component loaded into the production engine.
use super::lane_b::{Failures, Instance, Recorder, emit};
use super::support::local;
use maestro_extensions_wasmtime::test_support::maestro_extensions::SessionStartEvent;
use serde::Deserialize;
use serde_json::{Value, json};

/// The variable that names the built label component.
const COMPONENT: &str = "MAESTRO_HIDDEN_THINKING_LABEL_COMPONENT";

/// One registration the recorded metadata describes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Registration {
    /// The kind of registration.
    operation: String,
    /// Literal registration name.
    name: String,
    /// Supplied command description.
    #[serde(default)]
    description: Option<String>,
    /// Whether argument completions were supplied.
    #[serde(default)]
    completions: Option<bool>,
}

/// One recorded sequence of operations against a fresh component.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Recorded name of the sequence.
    name: String,
    /// The operations and their expected outcomes.
    steps: Vec<Step>,
}

/// One operation and what it must produce.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Step {
    /// The operation supplied to a registered callback.
    input: Input,
    /// The requests and error it must produce.
    expected: Expected,
}

/// The operation supplied to a registered callback.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Operation name: `session_start` or `command`.
    operation: String,
    /// Command argument text.
    #[serde(default)]
    args: Option<String>,
    /// Application rejection armed for one method.
    #[serde(default)]
    reject: Option<Rejection>,
}

/// A rejection armed for one method.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rejection {
    /// Method the rejection applies to.
    method: String,
    /// Message the application rejects that method with.
    message: String,
}

/// The requests a step must produce, and its error when it fails.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// UI requests in time order.
    calls: Vec<Call>,
    /// The error the step ends with.
    error: Option<String>,
}

/// One UI request: the method reached and the operands it received.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Call {
    /// Method the UI reached.
    method: String,
    /// Operands the UI received.
    operands: Value,
}

/// The recorded metadata and sequences.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Registrations as the example makes them.
    metadata: Vec<Registration>,
    /// The recorded sequences.
    cases: Vec<Case>,
}

/// The recorded corpus the tests consume.
fn corpus() -> Corpus {
    serde_json::from_str(include_str!("hidden_thinking_label_expected.json")).unwrap()
}

/// The recorded case named `name`.
fn case_of(name: &str) -> Case {
    corpus()
        .cases
        .into_iter()
        .find(|case| case.name == name)
        .unwrap()
}

/// One loaded component with the UI it reaches and the failures its callbacks report.
struct Session {
    /// The loaded component.
    instance: Instance,
    /// The recording UI.
    ui: std::rc::Rc<Recorder>,
    /// The failures the callbacks reported.
    failures: Failures,
}

impl Session {
    /// Loads a fresh component with a UI installed.
    async fn new() -> Self {
        let instance = Instance::new(COMPONENT).await;
        let ui = Recorder::install(instance.runner());
        let failures = Failures::collect(instance.runner());
        Self {
            instance,
            ui,
            failures,
        }
    }

    /// Delivers one recorded step and asserts the requests and ending it produces.
    async fn step(&self, case: &Case, index: usize, step: &Step) {
        if let Some(rejection) = &step.input.reject {
            self.ui.reject(&rejection.method, &rejection.message);
        }
        let runner = self.instance.runner();
        let error = if step.input.operation == "session_start" {
            emit::<SessionStartEvent>(runner, &json!({"reason": "new"})).await;
            self.failures.take().pop()
        } else {
            let command = runner.get_command("thinking-label").unwrap().command;
            let args = step.input.args.clone().unwrap_or_default();
            (command.handler)(args, runner.create_command_context())
                .await
                .err()
                .map(|error| error.to_string())
        };
        let calls: Vec<Value> = step
            .expected
            .calls
            .iter()
            .map(|call| json!([call.method, call.operands]))
            .collect();
        assert_eq!(self.ui.take(), calls, "{} step {index} requests", case.name);
        assert_eq!(
            error, step.expected.error,
            "{} step {index} error",
            case.name
        );
    }
}

/// Runs every step of one recorded case against a fresh component.
async fn checked_case(name: &str) {
    let case = case_of(name);
    let session = Session::new().await;
    for (index, step) in case.steps.iter().enumerate() {
        session.step(&case, index, step).await;
    }
}

/// Registers only the session hook and the command, then applies the current label at each start.
#[test]
fn hidden_thinking_label_registers_the_session_hook_and_command() {
    local(async {
        let session = Session::new().await;
        let corpus = corpus();
        let command = &corpus.metadata[1];
        assert_eq!(command.operation, "command");
        let runner = session.instance.runner();
        let commands = runner.get_registered_commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].command.name, command.name);
        assert_eq!(commands[0].command.description, command.description);
        assert_eq!(
            commands[0].command.get_argument_completions.is_some(),
            command.completions.unwrap_or(false)
        );
        assert!(runner.has_handlers("session_start"));
        assert!(session.ui.take().is_empty(), "registration requests no UI");
        let case = case_of("startup_and_repeat");
        for (index, step) in case.steps.iter().enumerate() {
            session.step(&case, index, step).await;
        }
    });
}

/// Every recorded custom label, reset and replay sequence produces its exact requests.
#[test]
fn hidden_thinking_label_commands_update_and_reset_shared_state() {
    local(async {
        for name in [
            "literal_custom_label",
            "empty_resets_then_start_reapplies_example_default",
            "ascii_whitespace_resets",
            "unicode_whitespace_resets",
            "unicode_whitespace_trims_custom",
            "interior_bytes_remain_literal",
            "nonwhitespace_neighbour_remains",
            "default_text_is_still_a_custom_set",
            "successive_custom_and_reset",
        ] {
            checked_case(name).await;
        }
    });
}

/// Every rejected request keeps the retained label and the order of the requests.
#[test]
fn hidden_thinking_label_failures_keep_state_and_order() {
    local(async {
        for name in [
            "start_rejection_is_returned",
            "custom_set_rejection_keeps_new_label",
            "reset_set_rejection_keeps_default",
            "custom_notify_rejection_keeps_new_label",
            "reset_notify_rejection_keeps_default",
        ] {
            checked_case(name).await;
        }
    });
}

/// A fresh component starts from the example default, whatever an earlier one was set to.
#[test]
fn hidden_thinking_label_instances_start_fresh() {
    local(async {
        let case = case_of("literal_custom_label");
        let first = Session::new().await;
        first.step(&case, 1, &case.steps[1]).await;
        drop(first);
        let startup = case_of("startup_and_repeat");
        let fresh = Session::new().await;
        fresh.step(&startup, 0, &startup.steps[0]).await;
    });
}

/// Every recorded sequence runs against a fresh guest of the built single-copy example.
#[test]
fn hidden_thinking_label_component_runs_registered_callbacks() {
    local(async {
        for case in corpus().cases {
            let session = Session::new().await;
            for (index, step) in case.steps.iter().enumerate() {
                session.step(&case, index, step).await;
            }
        }
    });
}
