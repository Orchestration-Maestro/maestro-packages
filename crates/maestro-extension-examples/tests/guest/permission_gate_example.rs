//! The shipped permission hook loaded into the production engine.
use super::support::{Recorder, TERMINAL, harness, load, local, tool_call};
use maestro_extensions::{ExtensionRunner, Local};
use serde_json::{Value, json};
use std::rc::Rc;

/// The variable that names the built permission component.
const COMPONENT: &str = "MAESTRO_PERMISSION_GATE_COMPONENT";

/// One tool call, the UI's decision and the verdict and prompt it must produce.
struct Case {
    /// The tool call document delivered to the hook.
    event: Value,
    /// Whether the host has an interactive UI.
    has_ui: bool,
    /// The choice the host returns, or null for dismissal.
    choice: Value,
    /// Whether a selection must be requested.
    asks: bool,
    /// The verdict the hook must give, null for none.
    verdict: Value,
}

/// A tool call with a literal, independent expected verdict.
fn case(tool: &str, command: &str, dangerous: bool, choice: Value) -> Case {
    let verdict = if dangerous && choice != "Yes" {
        json!({"block": true, "reason": "Blocked by user"})
    } else {
        Value::Null
    };
    Case {
        event: json!({"type":"tool_call","toolCallId":"gate-call","toolName":tool,
            "input":{"command":command,"untouched":777}}),
        has_ui: true,
        choice,
        asks: dangerous,
        verdict,
    }
}

/// Commands matched by the authored expressions, including their prefix and Unicode cases.
const DANGEROUS: &[&str] = &[
    "rm -r work",
    "rm -rf work",
    "RM\t--recursive work",
    "sudo whoami",
    "echo SUDO",
    "chmod 777 work",
    "chown root 777work",
    "chmod\t-R 777 work",
    "rm\u{feff}-r work",
    "ésudo whoami",
    "chmod\u{85}777 work",
    "rm -rubbish",
    "rm --recursively work",
];

/// Safe commands and near misses, including excluded line terminators.
const SAFE: &[&str] = &[
    "",
    "ls -la",
    "rm -f work",
    "rm -fr work",
    "arm -rf work",
    "_sudo",
    "sudoku",
    "chmod 755 work",
    "chown root work",
    "chmodx 777 work",
    "chmod\n777 work",
    "chown\r777 work",
    "chmod\u{2028}777 work",
    "chmod\u{2029}777 work",
    "rm\u{85}-rf work",
];

/// Authored pattern examples, near misses, UI outcomes and a rejected dialog.
fn cases() -> Vec<Case> {
    let mut rows = Vec::new();
    for command in DANGEROUS {
        for choice in [json!("Yes"), json!("No"), Value::Null, json!("yes")] {
            rows.push(case("bash", command, true, choice));
        }
        let mut headless = case("bash", command, true, json!("Yes"));
        headless.has_ui = false;
        headless.asks = false;
        headless.verdict = json!({"block":true,
            "reason":"Dangerous command blocked (no UI for confirmation)"});
        rows.push(headless);
    }
    for command in SAFE {
        rows.push(case("bash", command, false, json!("No")));
    }
    for tool in ["write", "edit", "custom", "Bash"] {
        let mut row = case(tool, "sudo rm -rf work", false, json!("No"));
        row.event["input"] = json!({"path":"sudo"});
        rows.push(row);
    }
    rows
}

/// Delivers one case and checks the prompt, the verdict and what the hook asked of the UI.
async fn check(ui: &Rc<Recorder>, runner: &Rc<ExtensionRunner<Local>>, row: &Case) {
    ui.connect(runner, row.has_ui);
    ui.reply(row.choice.clone());
    let verdict = tool_call(runner, &row.event).await;
    let requests = ui.take();
    let context = format!("{}; UI {}; choice {}", row.event, row.has_ui, row.choice);
    assert_eq!(verdict, Ok(row.verdict.clone()), "{context}");
    assert_eq!(requests.len(), usize::from(row.asks), "{context}");
    if row.asks {
        let command = row.event["input"]["command"].as_str().unwrap();
        assert_eq!(
            requests[0],
            json!([
                "select",
                [
                    format!("⚠️ Dangerous command:\n\n  {command}\n\nAllow?"),
                    ["Yes", "No"],
                    null,
                    null
                ]
            ]),
            "{context}"
        );
    }
}

/// Every recorded decision runs through the shipped factory's registered tool-call hook.
#[test]
fn permission_gate_hook_preserves_patterns_and_decisions() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let ui = Recorder::install_on(&loaded.runner, TERMINAL);
        for row in cases() {
            check(&ui, &loaded.runner, &row).await;
        }
    });
}

/// A selector that fails reaches the caller as the hook's own failure.
#[test]
fn permission_gate_selector_failure_reaches_the_caller() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let ui = Recorder::install_on(&loaded.runner, TERMINAL);
        ui.reject("select", "selector unavailable");
        let event = case("bash", "sudo whoami", true, Value::Null).event;
        assert_eq!(
            tool_call(&loaded.runner, &event).await,
            Err("selector unavailable".to_owned())
        );
    });
}
