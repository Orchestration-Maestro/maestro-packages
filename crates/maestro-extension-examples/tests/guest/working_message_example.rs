//! The shipped working-message startup component loaded into the production engine.
use super::support::{Failures, Instance, Recorder, emit, local};
use maestro_extensions_wasmtime::test_support::maestro_extensions::SessionStartEvent;
use serde::Deserialize;
use serde_json::{Value, json};

/// The variable that names the built startup component.
const COMPONENT: &str = "MAESTRO_WORKING_MESSAGE_TEST_COMPONENT";

/// The cases these tests consume.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Corpus {
    /// Every session-start reason and a repeated callback.
    requests: Vec<Row>,
    /// A refused working-text request and the callback after it.
    message_rejection: Vec<Row>,
    /// A refused indicator request and the callback after it.
    indicator_rejection: Vec<Row>,
}

/// One case: its name and the callback invocations it covers.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Name the case reports in a failed assertion.
    name: String,
    /// The callback invocations it covers, in order.
    steps: Vec<Step>,
}

/// One callback invocation and what it must produce.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Step {
    /// Event delivered to the registered callback.
    event: Value,
    /// The recorded UI owner; every step reaches the UI installed for it.
    binding: usize,
    /// Method whose request the application refuses.
    #[serde(default)]
    reject: Option<String>,
    /// Message the application refuses with.
    #[serde(default)]
    error: Option<String>,
    /// How the callback ends: `returned` or `rejected`.
    outcome: String,
    /// The requests the UI records, in order.
    requests: Vec<Request>,
}

/// One request the UI records.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    /// Method the UI reached.
    method: String,
    /// Operands the UI received.
    operands: Value,
}

/// The recorded cases.
fn corpus() -> Corpus {
    serde_json::from_str(include_str!("working_message_expected.json")).unwrap()
}

/// Delivers every step of `rows` to one loaded component, each to a freshly installed UI.
async fn replay(rows: &[Row]) {
    let instance = Instance::new(COMPONENT).await;
    let failures = Failures::collect(instance.runner());
    for row in rows {
        for step in &row.steps {
            let ui = Recorder::install(instance.runner());
            if let Some(method) = &step.reject {
                ui.reject(method, step.error.as_deref().unwrap());
            }
            emit::<SessionStartEvent>(instance.runner(), &step.event).await;
            let expected: Vec<Value> = step
                .requests
                .iter()
                .map(|request| json!([request.method, request.operands]))
                .collect();
            assert_eq!(
                ui.take(),
                expected,
                "{}: requests of binding {}",
                row.name,
                step.binding
            );
            let expected_failures: Vec<String> = match step.outcome.as_str() {
                "rejected" => vec![step.error.clone().unwrap()],
                _ => Vec::new(),
            };
            assert_eq!(
                failures.take(),
                expected_failures,
                "{}: the callback's ending",
                row.name
            );
        }
    }
}

/// Loading registers only the startup callback and requests nothing from the UI.
#[test]
fn working_message_registers_only_session_start() {
    local(async {
        let instance = Instance::new(COMPONENT).await;
        let ui = Recorder::install(instance.runner());
        assert!(instance.runner().has_handlers("session_start"));
        assert!(instance.runner().get_all_registered_tools().is_empty());
        assert!(instance.runner().get_registered_commands().is_empty());
        assert!(ui.take().is_empty(), "a request before the callback ran");
    });
}

/// Every session-start reason, and a repeated callback, request the same text and frame.
#[test]
fn working_message_requests_keep_style_and_order() {
    local(replay(&corpus().requests));
}

/// A refused working-text request stops the callback, and the registration then recovers.
#[test]
fn working_message_stops_after_message_rejection() {
    local(replay(&corpus().message_rejection));
}

/// A refused indicator request returns after the text request, and the registration recovers.
#[test]
fn working_message_returns_indicator_rejection() {
    local(replay(&corpus().indicator_rejection));
}

/// The built single-copy example runs every recorded case through its registration.
#[test]
fn working_message_component_matches_startup_requests() {
    local(async {
        let corpus = corpus();
        let all: Vec<Row> = [
            corpus.requests,
            corpus.message_rejection,
            corpus.indicator_rejection,
        ]
        .into_iter()
        .flatten()
        .collect();
        replay(&all).await;
    });
}
