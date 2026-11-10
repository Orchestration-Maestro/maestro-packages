//! The shipped timed dialogs loaded into the production engine.
use super::support::{TERMINAL, Recorder, driven, harness, load, local};
use serde_json::{Value, json};

/// The variable that names the built timed-dialog component.
const COMPONENT: &str = "MAESTRO_TIMED_CONFIRM_COMPONENT";

/// One host decision and its independently recorded notification.
struct Case {
    /// The registered command name.
    command: &'static str,
    /// The host's reply, not a decision made by the guest.
    answer: Value,
    /// The exact notification text and severity.
    notification: (&'static str, &'static str),
}

/// Confirmation and selection decisions with their literal expected output.
fn automatic_cases() -> Vec<Case> {
    [
        ("timed", json!(true), "Confirmed by user!"),
        ("timed", json!(false), "Cancelled or timed out"),
        ("timed-select", json!("Option A"), "Selected: Option A"),
        ("timed-select", json!("Option B"), "Selected: Option B"),
        ("timed-select", json!("Option C"), "Selected: Option C"),
        (
            "timed-select",
            Value::Null,
            "Selection cancelled or timed out",
        ),
        (
            "timed-select",
            json!(""),
            "Selection cancelled or timed out",
        ),
    ]
    .into_iter()
    .map(|(command, answer, message)| Case {
        command,
        answer,
        notification: (message, "Info"),
    })
    .collect()
}

/// Approval and manual cancellation of the dialog that carries a caller-owned signal.
fn signal_cases() -> Vec<Case> {
    [(true, "Confirmed by user!"), (false, "Cancelled by user")]
        .into_iter()
        .map(|(answer, message)| Case {
            command: "timed-signal",
            answer: json!(answer),
            notification: (message, "Info"),
        })
        .collect()
}

/// The requests a case must make, in order.
fn expected(case: &Case) -> Vec<Value> {
    let manual = case.command == "timed-signal";
    let mut requests = Vec::new();
    if manual {
        requests.push(json!([
            "notify",
            ["Dialog will auto-cancel in 5 seconds...", "Info"]
        ]));
    }
    requests.push(if case.command == "timed-select" {
        json!([
            "select",
            [
                "Pick an option",
                ["Option A", "Option B", "Option C"],
                null,
                10000.0
            ]
        ])
    } else {
        json!([
            "confirm",
            [
                "Timed Confirmation",
                "This dialog will auto-cancel in 5 seconds. Confirm?",
                manual.then_some(false),
                if manual { None } else { Some(5000.0) }
            ]
        ])
    });
    requests.push(json!([
        "notify",
        [case.notification.0, case.notification.1]
    ]));
    requests
}

/// Runs the registered command `name` and returns its failure, if any.
async fn run(
    loaded: &maestro_extensions_wasmtime::test_support::Loaded,
    name: &str,
) -> Option<String> {
    let command = loaded.runner.get_command(name).unwrap().command;
    (command.handler)(String::new(), loaded.runner.create_command_context())
        .await
        .err()
        .map(|error| error.to_string())
}

/// The three commands register in order with their countdown descriptions.
#[test]
fn timed_confirm_registers_commands_in_order() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let registered: Vec<_> = loaded
            .runner
            .get_registered_commands()
            .iter()
            .map(|command| {
                (
                    command.command.name.clone(),
                    command.command.description.clone(),
                )
            })
            .collect();
        assert_eq!(
            registered,
            [
                (
                    "timed".to_owned(),
                    Some(
                        "Show a timed confirmation dialog (auto-cancels in 5s with countdown)"
                            .to_owned()
                    )
                ),
                (
                    "timed-select".to_owned(),
                    Some(
                        "Show a timed select dialog (auto-cancels in 10s with countdown)"
                            .to_owned()
                    )
                ),
                (
                    "timed-signal".to_owned(),
                    Some("Show a timed confirm using AbortSignal (manual approach)".to_owned())
                ),
            ]
        );
    });
}

/// Confirmation and selection keep their timeouts and outcomes, and a refused dialog fails the
/// command.
#[test]
fn timed_confirm_commands_keep_timeouts_and_outcomes() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let ui = Recorder::install_on(&loaded.runner, TERMINAL);
        for case in automatic_cases() {
            ui.reply(case.answer.clone());
            assert_eq!(run(&loaded, case.command).await, None);
            assert_eq!(
                ui.take(),
                expected(&case),
                "{} {}",
                case.command,
                case.answer
            );
        }
        for (command, method) in [
            ("timed", "confirm"),
            ("timed-select", "select"),
            ("timed-signal", "confirm"),
        ] {
            ui.reject(method, "dialog refused");
            assert_eq!(
                run(&loaded, command).await,
                Some("dialog refused".to_owned())
            );
            ui.take();
        }
    });
}

/// The signal dialog tells approval and manual cancellation apart.
#[test]
fn timed_signal_distinguishes_approval_and_cancellation() {
    driven(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let ui = Recorder::install_on(&loaded.runner, TERMINAL);
        for case in signal_cases() {
            ui.reply(case.answer.clone());
            assert_eq!(run(&loaded, case.command).await, None);
            assert_eq!(
                ui.take(),
                expected(&case),
                "{} {}",
                case.command,
                case.answer
            );
        }
    });
}
