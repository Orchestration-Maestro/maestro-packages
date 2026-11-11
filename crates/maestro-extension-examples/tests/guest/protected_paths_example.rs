//! The shipped protected-paths hook loaded into the production engine.
use super::support::{Recorder, TERMINAL, harness, load, local, tool_call};
use serde_json::{Value, json};

/// The variable that names the built protected-paths component.
const COMPONENT: &str = "MAESTRO_PROTECTED_PATHS_COMPONENT";

/// Recorded delivery rows and host-failure probes.
fn corpus() -> Value {
    serde_json::from_str(include_str!("protected_paths_expected.json")).unwrap()
}

/// The notifications a recorded row expects, with the host's spelling of the severity.
fn expected_notifications(row: &Value) -> Value {
    row["notifications"]
        .as_array()
        .unwrap()
        .iter()
        .map(|expected| {
            assert_eq!(expected[1], "warning", "the recorded severity");
            json!(["notify", [expected[0], "Warning"]])
        })
        .collect()
}

/// The shipped hook registers one tool-call callback and no tool.
#[test]
fn protected_paths_registers_one_hook() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        assert!(loaded.runner.has_handlers("tool_call"));
        assert!(loaded.runner.get_all_registered_tools().is_empty());
    });
}

/// Every recorded tool call keeps its verdict and its notifications.
#[test]
fn protected_paths_matches_authored_paths() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let ui = Recorder::install_on(&loaded.runner, TERMINAL);
        let corpus = corpus();
        let rows = corpus["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 87, "recorded tool calls");
        for row in rows {
            ui.connect(&loaded.runner, row["hasUI"].as_bool().unwrap());
            let verdict = tool_call(&loaded.runner, &row["event"]).await;
            let name = &row["name"];
            assert_eq!(verdict, Ok(row["result"].clone()), "{name}");
            assert_eq!(json!(ui.take()), expected_notifications(row), "{name}");
        }
    });
}

/// A failing notification stays a catchable error, and an allowed call never reaches it.
#[test]
fn protected_paths_keeps_host_errors_catchable() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let ui = Recorder::install_on(&loaded.runner, TERMINAL);
        let corpus = corpus();
        let probes: Vec<&Value> = corpus["host_errors"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|probe| probe["target"] == "notify")
            .collect();
        assert_eq!(probes.len(), 3, "recorded notification probes");
        for probe in probes {
            ui.reject("notify", "controlled host failure");
            let event = json!({
                "type": "tool_call",
                "toolCallId": "controlled-call",
                "toolName": probe["toolName"],
                "input": {"path": probe["path"]},
            });
            let verdict = tool_call(&loaded.runner, &event).await;
            let name = format!("{} {}", probe["toolName"], probe["path"]);
            if probe["branch"] == "error" {
                assert_eq!(verdict, Err("controlled host failure".to_owned()), "{name}");
            } else {
                assert_eq!(verdict, Ok(Value::Null), "{name}");
            }
            assert_eq!(json!(ui.take()), expected_notifications(probe), "{name}");
        }
    });
}
