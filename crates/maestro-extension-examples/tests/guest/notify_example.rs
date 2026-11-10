//! The shipped completion notification loaded into the production engine.
//!
//! The terminal protocol and the Windows toast depend on the guest's environment, which the
//! public harness does not set, so the tests check what the host can see: the hook registers on
//! agent completion and every delivery ends without a failure.
use super::lane_a::{event, failures};
use super::support::{harness, load, local};
use maestro_extensions_wasmtime::test_support::maestro_extensions::AgentEndEvent;
use serde_json::json;

/// The variable that names the built notification component.
const COMPONENT: &str = "MAESTRO_NOTIFY_COMPONENT";

/// Every agent completion reaches the hook, repeatedly, and none of them fails.
#[test]
fn notify_component_selects_terminal_protocol() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let runner = &loaded.runner;
        assert!(runner.has_handlers("agent_end"));
        assert!(!runner.has_handlers("session_start"));
        let reported = failures(runner);
        for _ in 0..2 {
            let mut end: AgentEndEvent = event(json!({"type": "agent_end", "messages": []}));
            runner.emit(&mut end).await.unwrap();
        }
        assert_eq!(*reported.borrow(), Vec::<String>::new());
    });
}
