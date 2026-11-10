//! The shipped model-status factory loaded into the production engine.
use super::lane_c::{Dialogs, Failures, emit};
use super::support::{harness, load, local};
use maestro_extensions::ModelSelectEvent;
use serde_json::{Value, json};

/// The variable that names the built model-status component.
const COMPONENT: &str = "MAESTRO_MODEL_STATUS_COMPONENT";

/// Selection events, requests and outcomes recorded from the reference run.
fn corpus() -> Value {
    serde_json::from_str(include_str!("model_status_expected.json")).unwrap()
}

/// The recorded rows, with `error` selecting the host-rejection rows.
fn rows(error: bool) -> Vec<Value> {
    corpus()["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["outcome"]["error"].is_string() == error)
        .cloned()
        .collect()
}

/// The host's spelling of a recorded severity.
fn severity(text: &str) -> &str {
    match text {
        "info" => "Info",
        "warning" => "Warning",
        "error" => "Error",
        other => other,
    }
}

/// The UI requests a recorded row reaches, in order; the diagnostic written to standard output
/// is not a request.
fn reached(row: &Value) -> Vec<Value> {
    row["effects"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|effect| effect[0] != "stdout")
        .map(|effect| match effect[0].as_str().unwrap() {
            "notify" => json!(["notify", [effect[1], severity(effect[2].as_str().unwrap())]]),
            method => json!([method, [effect[1], effect[2]]]),
        })
        .collect()
}

/// The factory registers one selection hook and requests nothing while starting.
#[test]
fn model_status_registers_only_selection_hook() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let ui = Dialogs::install(&loaded.runner);
        assert_eq!(corpus()["registrations"], json!(["model_select"]));
        assert!(loaded.runner.has_handlers("model_select"));
        assert!(loaded.runner.get_all_registered_tools().is_empty());
        assert!(
            ui.take().is_empty(),
            "the factory requests no host UI while starting"
        );
    });
}

/// Every recorded successful selection keeps its literal identifiers and its requests.
#[test]
fn model_status_selection_preserves_literal_identifiers() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let ui = Dialogs::install(&loaded.runner);
        let failures = Failures::collect(&loaded.runner);
        let successes = rows(false);
        assert_eq!(successes.len(), 14, "recorded successes");
        for row in successes {
            let verdict = emit::<ModelSelectEvent>(&loaded.runner, &row["event"]).await;
            let name = &row["name"];
            assert_eq!(verdict, Ok(Value::Null), "{name}");
            assert_eq!(ui.take(), reached(&row), "{name}: requests");
            assert!(failures.take().is_empty(), "{name}: failures");
        }
    });
}

/// A rejected host request stops every later effect of the selection, and a later selection
/// still completes.
#[test]
fn model_status_host_rejections_stop_later_effects() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let ui = Dialogs::install(&loaded.runner);
        let failures = Failures::collect(&loaded.runner);
        for row in rows(true) {
            let error = row["outcome"]["error"].as_str().unwrap();
            ui.refuse_at(0, row["reject"].as_str().unwrap());
            let verdict = emit::<ModelSelectEvent>(&loaded.runner, &row["event"]).await;
            let name = &row["name"];
            assert_eq!(verdict, Ok(Value::Null), "{name}");
            assert_eq!(failures.take(), [error], "{name}: failure");
            assert_eq!(ui.take(), reached(&row), "{name}: requests");
        }
        let control = &rows(false)[0];
        emit::<ModelSelectEvent>(&loaded.runner, &control["event"])
            .await
            .unwrap();
        assert_eq!(ui.take(), reached(control), "the control still completes");
    });
}
