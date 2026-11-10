//! The shipped widget-placement factory loaded into the production engine.
use super::support::{TERMINAL, Failures, Recorder, verdict, harness, load, local};
use maestro_extensions::SessionStartEvent;
use maestro_extensions::{ExtensionRunner, Local};
use serde_json::{Value, json};
use std::rc::Rc;

/// The variable that names the built widget-placement component.
const COMPONENT: &str = "MAESTRO_WIDGET_PLACEMENT_COMPONENT";

/// The recorded requests and outcomes the tests consume.
fn corpus() -> Value {
    serde_json::from_str(include_str!("widget_placement_expected.json")).unwrap()
}

/// The request one recorded call makes, as the UI receives it; the host spells a placement
/// word with a capital.
fn request(call: &Value) -> Value {
    let placement = match call["options"]["placement"].as_str() {
        Some("belowEditor") => json!("BelowEditor"),
        Some(other) => panic!("unexpected placement {other}"),
        None => Value::Null,
    };
    json!(["setWidget", [call["key"], call["lines"], placement]])
}

/// The recorded requests that reach the UI with identity `owner`.
fn expected_for(calls: &Value, owner: u64) -> Vec<Value> {
    calls
        .as_array()
        .unwrap()
        .iter()
        .filter(|call| call["owner"] == owner)
        .map(request)
        .collect()
}

/// Arms the refusals and the rebinding one recorded step asks for.
fn arm(runner: &Rc<ExtensionRunner<Local>>, uis: &[Rc<Recorder>; 2], input: &Value) {
    if input["rebindAfterAbove"] == true {
        let (runner, next) = (runner.clone(), uis[1].clone());
        uis[0].after_first_request(move || next.connect(&runner, true));
    }
    if let Some(index) = input["rejectAt"].as_u64() {
        uis[0].reject_at(
            usize::try_from(index).unwrap(),
            input["error"].as_str().unwrap(),
        );
    }
}

/// Checks the requests each UI received against the recorded calls of its owner.
fn assert_requests(uis: &[Rc<Recorder>; 2], calls: &Value, name: &Value) {
    for (owner, ui) in (0..).zip(uis) {
        assert_eq!(
            ui.take(),
            expected_for(calls, owner),
            "{name} owner {owner}"
        );
    }
}

/// Runs the named corpus cases through the one registered session-start handler.
fn run(names: &[&str]) {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let runner = &loaded.runner;
        let failures = Failures::collect(runner);
        let uis = [Recorder::install_on(runner, TERMINAL), Recorder::install_on(runner, TERMINAL)];
        let corpus = corpus();
        let cases: Vec<&Value> = corpus["cases"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|case| names.contains(&case["name"].as_str().unwrap()))
            .collect();
        assert_eq!(cases.len(), names.len(), "the cases this test names");
        for case in cases {
            for step in case["steps"].as_array().unwrap() {
                let (name, input, expected) = (&case["name"], &step["input"], &step["expected"]);
                uis[0].connect(runner, input["hasUI"].as_bool().unwrap());
                arm(runner, &uis, input);
                let event = json!({"type": "session_start", "reason": "startup"});
                let verdict = verdict::<SessionStartEvent>(runner, &event).await;
                assert_eq!(verdict, Ok(Value::Null), "{name}");
                assert_requests(&uis, &expected["calls"], name);
                let reported = failures.take();
                match expected["outcome"]["error"].as_str() {
                    Some(error) => assert_eq!(reported, [error], "{name}"),
                    None => assert!(reported.is_empty(), "{name}: {reported:?}"),
                }
            }
        }
    });
}

/// Only the session-start handler is registered, and startup requests no widget.
#[test]
fn widget_placement_registers_only_session_start() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let ui = Recorder::install_on(&loaded.runner, TERMINAL);
        for name in corpus()["registrations"].as_array().unwrap() {
            assert!(loaded.runner.has_handlers(name.as_str().unwrap()));
        }
        assert!(loaded.runner.get_all_registered_tools().is_empty());
        assert!(ui.take().is_empty(), "widget requests during startup");
    });
}

/// Each start reads the live UI availability and requests both literal widgets.
#[test]
fn widget_placement_keeps_literals_and_rechecks_ui() {
    run(&[
        "headless_start",
        "interactive_start",
        "ui_presence_changes_between_starts",
        "ui_binding_changes_after_above",
    ]);
}

/// A rejected request ends the invocation with its own message and attempts nothing further.
#[test]
fn widget_placement_stops_on_host_rejection() {
    run(&["above_request_rejected", "below_request_rejected"]);
}
