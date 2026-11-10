//! The shipped user-message factory loaded into the production engine.
use super::support::{TERMINAL, Recorder, bind, harness, load, local};
use maestro_extensions_wasmtime::test_support::{Harness, Loaded};
use serde_json::{Value, json};
use std::{cell::Cell, rc::Rc};

/// The variable that names the built user-message component.
const COMPONENT: &str = "MAESTRO_SEND_USER_MESSAGE_COMPONENT";

/// The argument classes that carry no message.
const BLANK: [&str; 3] = [
    "",
    " \t\r\n ",
    "\u{a0}\u{1680}\u{2003}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}",
];

/// The argument classes native whitespace decides about: the byte-order mark is text and the
/// next-line character is blank.
const NATIVE_BLANK: [&str; 2] = ["\u{feff}", "\u{85}"];

/// The argument class that carries text with the whitespace around it.
const MESSAGE: &str = "  What is 雪 + e\u{301}?\r\n/steer literal  ";

/// The argument class that carries one zero-width character.
const ZERO_WIDTH: &str = "\u{200b}";

/// The recorded group name of the refused-operation cases.
const FAILURES: &str = "failures";

/// Recorded registrations and cases.
fn corpus() -> Value {
    serde_json::from_str(include_str!("send_user_message_expected.json")).unwrap()
}

/// The host operations a case refuses.
#[derive(Default)]
struct Refusals {
    /// Whether the idle check fails.
    idle: Cell<bool>,
    /// Whether the delivery fails.
    send: Cell<bool>,
}

impl Refusals {
    /// Fails with `message` while `flag` is set.
    fn fail(flag: &Cell<bool>, message: &str) -> Result<(), maestro_extensions::EventError> {
        if flag.get() {
            Err(message.into())
        } else {
            Ok(())
        }
    }
}

/// One loaded instance whose idle flag, delivery and notifications the test controls.
struct Instance {
    /// Keeps the engine alive.
    _harness: Harness,
    /// The loaded runner.
    loaded: Loaded,
    /// The UI and the log of host effects.
    ui: Rc<Recorder>,
    /// Whether the agent is idle.
    idle: Rc<Cell<bool>>,
    /// The refused operations.
    refusals: Rc<Refusals>,
}

impl Instance {
    /// Loads the component and serves the idle check and the delivery.
    async fn new() -> Self {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let ui = Recorder::install_on(&loaded.runner, TERMINAL);
        let idle = Rc::new(Cell::new(true));
        let refusals = Rc::new(Refusals::default());
        bind(&loaded.runner, |actions, context| {
            let (log, refused) = (ui.clone(), refusals.clone());
            actions.send_user_message = Rc::new(move |content, options| {
                Refusals::fail(&refused.send, "delivery refused")?;
                log.record("sendUserMessage", json!([content, options]))
            });
            let (state, refused) = (idle.clone(), refusals.clone());
            context.is_idle = Rc::new(move || {
                Refusals::fail(&refused.idle, "idle refused")?;
                Ok(state.get())
            });
        });
        Self {
            _harness: harness,
            loaded,
            ui,
            idle,
            refusals,
        }
    }

    /// The names of the registered commands, in order.
    fn registered(&self) -> Vec<String> {
        self.loaded
            .runner
            .get_registered_commands()
            .iter()
            .map(|command| command.command.name.clone())
            .collect()
    }

    /// Runs one recorded case and checks every effect and the failure it records.
    async fn check(&self, case: &Value) {
        let (name, input) = (&case["name"], &case["input"]);
        self.ui.disarm();
        self.idle.set(input["idle"].as_bool().unwrap());
        let reject = input["reject"].as_str();
        self.refusals.idle.set(reject == Some("idle"));
        self.refusals.send.set(reject == Some("send"));
        if reject == Some("notify") {
            self.ui.reject("notify", "notification refused");
        }
        let command = self
            .loaded
            .runner
            .get_command(input["command"].as_str().unwrap())
            .unwrap()
            .command;
        let outcome = (command.handler)(
            input["args"].as_str().unwrap().into(),
            self.loaded.runner.create_command_context(),
        )
        .await;
        let expected = &case["expected"];
        assert_eq!(
            json!(self.registered()),
            expected["registration"],
            "{name}: registrations"
        );
        let calls: Vec<Value> = expected["calls"]
            .as_array()
            .unwrap()
            .iter()
            .map(request)
            .collect();
        assert_eq!(self.ui.take(), calls, "{name}: calls");
        let error = outcome.err().map(|error| error.to_string());
        assert_eq!(
            error.as_deref(),
            expected["error"].as_str(),
            "{name}: answer"
        );
    }
}

/// The request one recorded effect is, in the log's spelling.
fn request(call: &Value) -> Value {
    if call["operation"] == "notify" {
        let kind = match call["kind"].as_str().unwrap() {
            "warning" => "Warning",
            "info" => "Info",
            other => panic!("unexpected severity {other}"),
        };
        json!(["notify", [call["message"], kind]])
    } else {
        json!(["sendUserMessage", [call["content"], call["options"]]])
    }
}

/// The recorded cases `keep` selects, which never register a refusal.
fn rows(keep: impl Fn(&Value) -> bool) -> Vec<Value> {
    corpus()["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| keep(row) && !registration_refused(row))
        .cloned()
        .collect()
}

/// Whether the case refuses a registration.
fn registration_refused(row: &Value) -> bool {
    row["input"]["reject"]
        .as_str()
        .is_some_and(|reject| reject.starts_with("register:"))
}

/// Whether a case belongs to the ordinary groups and refuses nothing.
fn ordinary(row: &Value) -> bool {
    row["group"] != FAILURES && row["input"]["reject"].is_null()
}

/// Whether a case's argument is one of `classes`.
fn args_in(row: &Value, classes: &[&str]) -> bool {
    classes.contains(&row["input"]["args"].as_str().unwrap())
}

/// Replays the recorded message cases of `command`.
fn replay_messages(command: &str, count: usize) {
    local(async {
        let instance = Instance::new().await;
        let cases = rows(|row| {
            ordinary(row)
                && row["input"]["command"] == command
                && args_in(row, &[MESSAGE, ZERO_WIDTH])
        });
        assert_eq!(cases.len(), count, "the rows of the recorded group");
        for case in &cases {
            instance.check(case).await;
        }
    });
}

/// Every command is registered with its own description and no completions, in order.
#[test]
fn user_delivery_registers_commands_in_order() {
    local(async {
        let instance = Instance::new().await;
        let registered: Vec<Value> = instance
            .loaded
            .runner
            .get_registered_commands()
            .iter()
            .map(|command| {
                json!({
                    "name": command.command.name,
                    "description": command.command.description,
                    "completions": command.command.get_argument_completions.is_some(),
                })
            })
            .collect();
        assert_eq!(json!(registered), corpus()["metadata"]);
        assert!(instance.loaded.runner.get_all_registered_tools().is_empty());
    });
}

/// A blank argument warns before any delivery, whichever command receives it.
#[test]
fn user_delivery_blank_arguments_warn_before_delivery() {
    local(async {
        let instance = Instance::new().await;
        let cases = rows(|row| ordinary(row) && args_in(row, &BLANK));
        assert_eq!(cases.len(), 24, "the blank rows");
        for case in &cases {
            instance.check(case).await;
        }
    });
}

/// The ordinary command forwards the original text while idle and warns while busy.
#[test]
fn user_delivery_ask_checks_idle_and_keeps_text() {
    replay_messages("ask", 4);
}

/// Steering selects its own delivery only while the agent is busy.
#[test]
fn user_delivery_steer_selects_only_while_busy() {
    replay_messages("steer", 4);
}

/// A follow-up names its delivery and reports the queue only after the host admitted it.
#[test]
fn user_delivery_followup_reports_only_the_queued_branch() {
    replay_messages("followup", 4);
}

/// The structured command sends two text blocks in order while idle and warns while busy.
#[test]
fn user_delivery_askwith_keeps_two_text_blocks() {
    replay_messages("askwith", 4);
}

/// Boundary whitespace follows the native trim, which keeps the byte-order mark and drops the
/// next-line character.
#[test]
fn user_delivery_blank_checks_use_native_whitespace() {
    local(async {
        let instance = Instance::new().await;
        let cases = rows(|row| ordinary(row) && args_in(row, &NATIVE_BLANK));
        assert_eq!(cases.len(), 16, "the boundary-whitespace rows");
        for case in &cases {
            instance.check(case).await;
        }
    });
}

/// A refused host operation stops the invocation and keeps the effects before it.
#[test]
fn user_delivery_rejections_stop_at_the_failed_operation() {
    local(async {
        let instance = Instance::new().await;
        let cases = rows(|row| row["group"] == FAILURES);
        assert_eq!(cases.len(), 21, "the failure rows");
        for case in &cases {
            instance.check(case).await;
        }
    });
}
