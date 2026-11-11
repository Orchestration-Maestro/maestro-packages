//! The shipped turn status hooks loaded into the production engine.
use super::support::{Failures, Recorder, event, harness, load, local, plain_text};
use maestro_extensions_wasmtime::test_support::{Harness, Loaded, maestro_extensions::*};
use serde_json::json;
use std::rc::Rc;

/// The variable that names the built status component.
const COMPONENT: &str = "MAESTRO_STATUS_LINE_COMPONENT";

/// One loaded instance with the recording UI its status requests reach.
struct Instance {
    /// The engine the instance lives in.
    harness: Harness,
    /// The runner over the loaded component.
    loaded: Loaded,
    /// The requests the component made.
    ui: Rc<Recorder>,
    /// The callback failures the runner reported.
    failures: Failures,
}

impl Instance {
    /// Loads one independent instance.
    async fn new() -> Self {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let ui = Recorder::install(&loaded.runner);
        let failures = Failures::collect(&loaded.runner);
        Self {
            harness,
            loaded,
            ui,
            failures,
        }
    }

    /// Delivers one lifecycle event and returns the status text it set, without styling.
    async fn deliver(&self, name: &str) -> Vec<String> {
        let runner = &self.loaded.runner;
        match name {
            "session_start" => {
                let mut event: SessionStartEvent =
                    event(json!({"type": "session_start", "reason": "startup"}));
                runner.emit(&mut event).await.unwrap();
            }
            "turn_start" => {
                let mut event: TurnStartEvent =
                    event(json!({"type": "turn_start", "turnIndex": 0, "timestamp": 0}));
                runner.emit(&mut event).await.unwrap();
            }
            _ => panic!("no event named {name}"),
        }
        self.statuses()
    }

    /// The plain text of every status request recorded since the last call.
    fn statuses(&self) -> Vec<String> {
        self.ui
            .take()
            .iter()
            .map(|request| {
                assert_eq!(request[0], "setStatus");
                assert_eq!(request[1][0], "status-demo");
                plain_text(request[1][1].as_str().unwrap())
            })
            .collect()
    }
}

/// Each session start sets the ready text under the authored key.
#[test]
fn status_line_component_sets_ready_on_every_session_start() {
    local(async {
        let instance = Instance::new().await;
        for _ in 0..2 {
            assert_eq!(instance.deliver("session_start").await, ["Ready"]);
        }
        assert!(instance.failures.seen().is_empty());
        drop(instance.harness);
    });
}

/// The factory counts turn starts, and a new session does not reset the count.
#[test]
fn status_line_component_counts_turns_across_sessions() {
    local(async {
        let instance = Instance::new().await;
        let (start, session) = ("turn_start", "session_start");
        assert_eq!(instance.deliver(start).await, ["● Turn 1..."]);
        assert_eq!(instance.deliver(session).await, ["Ready"]);
        assert_eq!(instance.deliver(start).await, ["● Turn 2..."]);
        assert!(instance.failures.seen().is_empty());
        let fresh = Instance::new().await;
        assert_eq!(fresh.deliver(start).await, ["● Turn 1..."]);
    });
}

/// A refused status request is reported and does not undo the started turn.
#[test]
fn status_line_component_preserves_count_after_host_rejection() {
    local(async {
        let instance = Instance::new().await;
        instance.ui.reject("setStatus", "host refused");
        assert_eq!(instance.deliver("turn_start").await, ["● Turn 1..."]);
        assert_eq!(instance.failures.seen(), ["host refused"]);
        assert_eq!(instance.deliver("turn_start").await, ["● Turn 2..."]);
    });
}
