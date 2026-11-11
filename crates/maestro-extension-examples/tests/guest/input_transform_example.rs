//! The shipped input hook loaded into the production engine.
use super::support::{Failures, Recorder, harness, load, local};
use maestro_extensions_wasmtime::test_support::maestro_extensions::{InputSource, Presence};
use serde_json::{Value, json};

/// The variable that names the built input component.
const COMPONENT: &str = "MAESTRO_INPUT_TRANSFORM_COMPONENT";

/// What one input produced: the hook's decision, its notifications and the failures it reported.
struct Outcome {
    /// The decision in the host's own document form.
    decision: Value,
    /// Each notification as the message and severity.
    notifications: Vec<Value>,
    /// The callback failures the runner reported.
    failures: Vec<String>,
}

/// Delivers each `(source, text)` input to a freshly loaded component, one outcome per input.
fn inputs(rows: &[(InputSource, &str)], rejecting: Option<&str>) -> Vec<Outcome> {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        assert!(loaded.runner.has_handlers("input"));
        let ui = Recorder::install(&loaded.runner);
        let reported = Failures::collect(&loaded.runner);
        let mut outcomes = Vec::new();
        for (source, text) in rows {
            if let Some(message) = rejecting {
                ui.reject("notify", message);
            }
            let mut images = Presence::Missing;
            let decision = loaded
                .runner
                .emit_input(text, &mut images, *source, None)
                .await
                .unwrap();
            assert!(matches!(images, Presence::Missing), "{text:?}: images");
            outcomes.push(Outcome {
                decision: serde_json::to_value(decision).unwrap(),
                notifications: ui.take().iter().map(|request| request[1].clone()).collect(),
                failures: reported.take(),
            });
        }
        outcomes
    })
}

/// Injected input bypasses every local action; ordinary near misses continue without trimming.
#[test]
fn input_transform_bypasses_extension_and_continues_other_input() {
    let mut rows = Vec::new();
    for text in ["?quick explain", "?quick ", "PING", "time"] {
        rows.push((InputSource::Extension, text));
    }
    for source in [InputSource::Interactive, InputSource::Rpc] {
        for text in [
            "",
            "hello",
            "?quick",
            "?Quick question",
            " ping",
            "ping ",
            "time\n",
        ] {
            rows.push((source, text));
        }
    }
    for (row, outcome) in rows.iter().zip(inputs(&rows, None)) {
        assert_eq!(outcome.decision, json!({"action": "continue"}), "{row:?}");
        assert!(outcome.notifications.is_empty(), "{row:?}");
        assert!(outcome.failures.is_empty(), "{row:?}");
    }
}

/// Only the literal prefix rewrites; shared whitespace trims the edges but preserves interiors.
#[test]
fn input_transform_rewrites_quick_questions() {
    let cases = [
        (
            "?quick What is Rust?",
            "Respond briefly in 1-2 sentences: What is Rust?",
        ),
        (
            "?quick \u{feff}\t A  😀\nB \u{a0}\u{2028}",
            "Respond briefly in 1-2 sentences: A  😀\nB",
        ),
        (
            "?quick \u{85} question \u{85}",
            "Respond briefly in 1-2 sentences: \u{85} question \u{85}",
        ),
    ];
    let mut rows = Vec::new();
    for source in [InputSource::Interactive, InputSource::Rpc] {
        for (text, _) in cases {
            rows.push((source, text));
        }
    }
    let outcomes = inputs(&rows, None);
    for (index, outcome) in outcomes.iter().enumerate() {
        let expected = cases[index % cases.len()].1;
        assert_eq!(
            outcome.decision,
            json!({"action": "transform", "text": expected}),
            "{:?}",
            rows[index]
        );
        assert!(outcome.notifications.is_empty());
    }
}

/// Empty quick input warns and stops rather than producing an empty model request, and a
/// refused warning is reported without turning the input into a request.
#[test]
fn input_transform_handles_empty_quick_input() {
    let rows: Vec<_> = ["?quick ", "?quick \t\u{feff}\n\u{a0}\u{2028}"]
        .into_iter()
        .map(|text| (InputSource::Interactive, text))
        .collect();
    for outcome in inputs(&rows, None) {
        assert_eq!(outcome.decision, json!({"action": "handled"}));
        assert_eq!(
            outcome.notifications,
            [json!(["Usage: ?quick <question>", "Warning"])]
        );
    }
    let refused = inputs(
        &[(InputSource::Rpc, "?quick ")],
        Some("notification unavailable"),
    );
    assert_eq!(refused[0].decision, json!({"action": "continue"}));
    assert_eq!(
        refused[0].notifications,
        [json!(["Usage: ?quick <question>", "Warning"])]
    );
    assert_eq!(refused[0].failures, ["notification unavailable"]);
}

/// Ping ignores ASCII case and shows one local informational notification.
#[test]
fn input_transform_handles_ping_locally() {
    let rows: Vec<_> = ["ping", "PING", "PiNg"]
        .into_iter()
        .map(|text| (InputSource::Rpc, text))
        .collect();
    for outcome in inputs(&rows, None) {
        assert_eq!(outcome.decision, json!({"action": "handled"}));
        assert_eq!(outcome.notifications, [json!(["pong", "Info"])]);
    }
    let refused = inputs(
        &[(InputSource::Interactive, "ping")],
        Some("notification unavailable"),
    );
    assert_eq!(refused[0].failures, ["notification unavailable"]);
    assert_eq!(refused[0].notifications, [json!(["pong", "Info"])]);
}

/// Time shows the host's local date and time as one informational notification.
#[test]
fn input_transform_handles_time_from_the_host_clock() {
    let rows: Vec<_> = ["time", "TIME", "TiMe"]
        .into_iter()
        .map(|text| (InputSource::Interactive, text))
        .collect();
    for outcome in inputs(&rows, None) {
        assert_eq!(outcome.decision, json!({"action": "handled"}));
        let [notification] = outcome.notifications.as_slice() else {
            panic!("one notification: {:?}", outcome.notifications);
        };
        assert_eq!(notification[1], "Info");
        let shown = notification[0].as_str().unwrap();
        assert!(
            shown.ends_with(" AM") || shown.ends_with(" PM"),
            "a 12-hour local date and time: {shown}"
        );
    }
    let refused = inputs(
        &[(InputSource::Rpc, "TiMe")],
        Some("notification unavailable"),
    );
    assert_eq!(refused[0].failures, ["notification unavailable"]);
}
