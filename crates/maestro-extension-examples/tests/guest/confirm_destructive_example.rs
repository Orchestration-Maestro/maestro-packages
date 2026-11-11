//! The shipped session confirmations loaded into the production engine.
use super::support::{Failures, Recorder, TERMINAL, harness, load, local, verdict};
use maestro_extensions::{
    ExtensionRunner, Local, SessionBeforeForkEvent, SessionBeforeSwitchEvent, SessionManager,
};
use serde_json::{Value, json};
use std::rc::Rc;

/// The variable that names the built confirmation component.
const COMPONENT: &str = "MAESTRO_CONFIRM_DESTRUCTIVE_COMPONENT";

/// One session event, history, dialog answer and the exact outcome it must produce.
struct Case {
    /// The event document delivered to the handler.
    event: Value,
    /// The current branch, as message entries.
    messages: Vec<Value>,
    /// Whether UI is available.
    has_ui: bool,
    /// The dialog's answer.
    decision: Value,
    /// The exact ordered UI requests.
    requests: Value,
    /// The verdict, null when the handler gave none.
    verdict: Value,
    /// The UI method that fails with `UI refused`.
    refuse: Option<&'static str>,
}

/// An assistant message.
fn assistant() -> Value {
    json!({"role":"assistant","content":[],"api":"controlled","provider":"controlled",
        "model":"controlled","usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,
            "totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},
        "stopReason":"stop","timestamp":0})
}

/// Literal user history followed by an assistant reply.
fn history() -> Vec<Value> {
    vec![
        json!({"role":"user","content":"hello","timestamp":0}),
        assistant(),
    ]
}

/// A request to the host dialog, in the operand shape the UI records.
fn request(method: &str, title: &str, second: &Value) -> Value {
    json!([method, [title, second, null, null]])
}

/// Replacement behavior with empty, non-user and user history, approval, dismissal and host
/// failures.
fn switch_dialog_cases() -> Vec<Case> {
    let mut rows = Vec::new();
    for (reason, messages, title, message, cancelled) in [
        (
            "new",
            Vec::new(),
            "Clear session?",
            "This will delete all messages in the current session.",
            "Clear cancelled",
        ),
        (
            "new",
            history(),
            "Clear session?",
            "This will delete all messages in the current session.",
            "Clear cancelled",
        ),
        (
            "resume",
            history(),
            "Switch session?",
            "You have messages in the current session. Switch anyway?",
            "Switch cancelled",
        ),
    ] {
        for decision in [json!(true), json!(false), Value::Null] {
            let mut requests = vec![request("confirm", title, &json!(message))];
            let verdict = if decision == true {
                Value::Null
            } else {
                requests.push(json!(["notify", [cancelled, "Info"]]));
                json!({"cancel":true})
            };
            rows.push(Case {
                event: json!({"type":"session_before_switch","reason":reason,"targetSessionFile":"supplied.jsonl"}),
                messages: messages.clone(),
                has_ui: true,
                decision,
                requests: json!(requests),
                verdict,
                refuse: None,
            });
        }
    }
    rows
}

/// Headless delivery and histories without user messages never ask.
fn switch_bypass_cases() -> Vec<Case> {
    let mut rows = Vec::new();
    for reason in ["new", "resume"] {
        rows.push(Case {
            event: json!({"type":"session_before_switch","reason":reason}),
            messages: history(),
            has_ui: false,
            decision: json!(false),
            requests: json!([]),
            verdict: Value::Null,
            refuse: None,
        });
    }
    for messages in [Vec::new(), vec![assistant()]] {
        rows.push(Case {
            event: json!({"type":"session_before_switch","reason":"resume"}),
            messages,
            has_ui: true,
            decision: json!(false),
            requests: json!([]),
            verdict: Value::Null,
            refuse: None,
        });
    }
    rows
}

/// Dialog and notification failures remain caller-visible errors.
fn switch_refusal_cases() -> Vec<Case> {
    let confirm = request(
        "confirm",
        "Clear session?",
        &json!("This will delete all messages in the current session."),
    );
    ["confirm", "notify"]
        .into_iter()
        .map(|method| Case {
            event: json!({"type":"session_before_switch","reason":"new"}),
            messages: Vec::new(),
            has_ui: true,
            decision: json!(false),
            requests: if method == "confirm" {
                json!([confirm])
            } else {
                json!([confirm, ["notify", ["Clear cancelled", "Info"]]])
            },
            verdict: Value::Null,
            refuse: Some(method),
        })
        .collect()
}

/// Fork approval is exact, independent of fork position, and shows only the ID prefix.
fn fork_dialog_cases() -> Vec<Case> {
    let choices = ["Yes, create fork", "No, stay in current session"];
    let mut rows = Vec::new();
    for (id, title, position) in [
        ("123456789abcdef", "Fork from entry 12345678?", "at"),
        ("short", "Fork from entry short?", "at"),
        ("", "Fork from entry ?", "at"),
        ("123456789abcdef", "Fork from entry 12345678?", "before"),
    ] {
        for decision in [
            json!("Yes, create fork"),
            json!("No, stay in current session"),
            Value::Null,
            json!("Yes, create fork please"),
            json!("yes, create fork"),
        ] {
            let mut requests = vec![request("select", title, &json!(choices))];
            let verdict = if decision == "Yes, create fork" {
                Value::Null
            } else {
                requests.push(json!(["notify", ["Fork cancelled", "Info"]]));
                json!({"cancel":true})
            };
            rows.push(Case {
                event: json!({"type":"session_before_fork","entryId":id,"position":position}),
                messages: Vec::new(),
                has_ui: true,
                decision,
                requests: json!(requests),
                verdict,
                refuse: None,
            });
        }
    }
    rows
}

/// A headless host continues; refused selectors and notifications propagate their errors.
fn fork_cases() -> Vec<Case> {
    let mut rows = fork_dialog_cases();
    for position in ["at", "before"] {
        rows.push(Case {
            event: json!({"type":"session_before_fork","entryId":"headless","position":position}),
            messages: Vec::new(),
            has_ui: false,
            decision: Value::Null,
            requests: json!([]),
            verdict: Value::Null,
            refuse: None,
        });
    }
    for method in ["select", "notify"] {
        let mut requests = vec![request(
            "select",
            "Fork from entry rejected?",
            &json!(["Yes, create fork", "No, stay in current session"]),
        )];
        if method == "notify" {
            requests.push(json!(["notify", ["Fork cancelled", "Info"]]));
        }
        rows.push(Case {
            event: json!({"type":"session_before_fork","entryId":"rejected","position":"at"}),
            messages: Vec::new(),
            has_ui: true,
            decision: Value::Null,
            requests: json!(requests),
            verdict: Value::Null,
            refuse: Some(method),
        });
    }
    rows
}

/// Delivers each case to a freshly loaded component and checks the requests, the verdict and
/// the failure the host reported.
async fn deliver<V>(cases: Vec<Case>)
where
    V: maestro_extensions::execution::RunnerEmitEvent + serde::de::DeserializeOwned,
    V::Result: serde::Serialize,
{
    let harness = harness();
    let loaded = load(&harness, COMPONENT).await;
    let runner: &Rc<ExtensionRunner<Local>> = &loaded.runner;
    let ui = Recorder::install_on(runner, TERMINAL);
    let failures = Failures::collect(runner);
    for row in cases {
        let mut history = SessionManager::in_memory(Some("/work")).unwrap();
        for message in &row.messages {
            history.append_message(message.clone()).unwrap();
        }
        *harness.session_manager.borrow_mut() = history;
        ui.connect(runner, row.has_ui);
        ui.reply(row.decision.clone());
        if let Some(method) = row.refuse {
            ui.reject(method, "UI refused");
        }
        let verdict = verdict::<V>(runner, &row.event).await;
        let name = row.event.to_string();
        assert_eq!(
            ui.take(),
            row.requests.as_array().unwrap().clone(),
            "{name}"
        );
        let refused = failures.take();
        if row.refuse.is_some() {
            assert_eq!(refused, ["UI refused"], "{name}");
        } else {
            assert!(refused.is_empty(), "{name}: {refused:?}");
            assert_eq!(verdict, Ok(row.verdict.clone()), "{name}");
        }
    }
}

/// Replacement decisions retain their exact prompts and cancellation outcomes.
#[test]
fn confirm_destructive_switch_decisions() {
    local(async {
        let cases = switch_dialog_cases()
            .into_iter()
            .chain(switch_bypass_cases())
            .chain(switch_refusal_cases())
            .collect();
        deliver::<SessionBeforeSwitchEvent>(cases).await;
    });
}

/// Forks need the exact affirmative choice unless no UI is available.
#[test]
fn confirm_destructive_fork_decisions() {
    local(async { deliver::<SessionBeforeForkEvent>(fork_cases()).await });
}
