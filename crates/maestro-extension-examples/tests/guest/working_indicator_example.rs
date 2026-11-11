//! The shipped working-indicator command loaded into the production engine.
//!
//! Status text is compared without the theme's styling, and a refused theme lookup, which the
//! public UI cannot express, is replayed as an accepted one whose first request must match.
use super::support::{Failures, Recorder, event, harness, load, local, plain_text};
use maestro_extensions_wasmtime::test_support::maestro_extensions::SessionStartEvent;
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// The variable that names the built indicator component.
const COMPONENT: &str = "MAESTRO_WORKING_INDICATOR_COMPONENT";

/// The recorded invocation groups, each starting with a fresh factory.
fn corpus() -> BTreeMap<String, Vec<Value>> {
    serde_json::from_str(include_str!("working_indicator_expected.json")).unwrap()
}

/// The requests of a recorded step without theme lookups and with status text unstyled.
fn requests(recorded: &[Value]) -> Vec<Value> {
    recorded
        .iter()
        .filter(|request| request[0] != "theme")
        .map(|request| {
            let mut request = request.clone();
            if request[0] == "setStatus" {
                let text = plain_text(request[1][1].as_str().unwrap());
                request[1][1] = json!(text);
            }
            request
        })
        .collect()
}

/// Replays every step of one recorded group against one loaded component.
fn replay(group: &str) {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let runner = &loaded.runner;
        assert!(runner.has_handlers("session_start"));
        let ui = Recorder::install(runner);
        let reported = Failures::collect(runner);
        for step in &corpus()[group] {
            let rejected = step["reject"].as_str();
            if let Some(method) = rejected.filter(|method| *method != "theme") {
                ui.reject(method, "refused");
            }
            let error = if let Some(document) = step.get("event") {
                let mut start: SessionStartEvent = event(document.clone());
                runner.emit(&mut start).await.unwrap();
                reported.take().pop()
            } else {
                let command = runner.get_command("working-indicator").unwrap().command;
                (command.handler)(
                    step["args"].as_str().unwrap().into(),
                    runner.create_command_context(),
                )
                .await
                .err()
                .map(|error| error.to_string())
            };
            let seen = requests(&ui.take());
            let recorded: Vec<Value> = step["requests"]
                .as_array()
                .unwrap()
                .iter()
                .map(|request| json!([request["method"], request["operands"]]))
                .collect();
            let expected = requests(&recorded);
            if rejected == Some("theme") {
                assert_eq!(seen[..1], expected[..1], "{step}");
                continue;
            }
            assert_eq!(seen, expected, "{step}");
            assert_eq!(error.as_deref(), rejected.map(|_| "refused"), "{step}");
        }
    });
}

/// Startup applies the initially selected custom spinner and its dim status.
#[test]
fn working_indicator_startup_applies_custom_spinner() {
    replay("startup");
}

/// Every named mode keeps its frames, interval, description and themed status.
#[test]
fn working_indicator_modes_keep_frames_intervals_and_status() {
    replay("modes");
}

/// Queries and invalid arguments leave the selected mode unchanged.
#[test]
fn working_indicator_queries_and_invalid_input_leave_selection_unchanged() {
    replay("queries");
}

/// Host refusals keep the selection and stop later UI requests.
#[test]
fn working_indicator_refusals_preserve_selection_and_request_order() {
    replay("refusals");
}

/// The built component registers exactly its startup hook and its one command.
#[test]
fn working_indicator_component_matches_registered_callbacks() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let commands = loaded.runner.get_registered_commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].command.name, "working-indicator");
        assert_eq!(
            commands[0].command.description.as_deref(),
            Some("Set the streaming working indicator: dot, pulse, none, spinner, or reset.")
        );
        assert!(commands[0].command.get_argument_completions.is_none());
        assert!(loaded.runner.has_handlers("session_start"));
    });
}
