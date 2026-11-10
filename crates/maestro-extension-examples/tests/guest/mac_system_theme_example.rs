//! The shipped appearance follower's lifecycle, loaded into the production engine.
use super::lane_c::{Dialogs, emit};
use super::support::{harness, load, local};
use maestro_extensions::SessionShutdownEvent;
use serde_json::{Value, json};

/// The variable that names the built appearance-follower component.
const COMPONENT: &str = "MAESTRO_MAC_SYSTEM_THEME_COMPONENT";

/// The follower registers its start and shutdown handlers and touches the UI only when a
/// session starts.
#[test]
fn mac_system_theme_registers_lifecycle_handlers() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let ui = Dialogs::install(&loaded.runner);
        for event in ["session_start", "session_shutdown"] {
            assert!(loaded.runner.has_handlers(event), "{event}");
        }
        assert!(loaded.runner.get_all_registered_tools().is_empty());
        assert!(
            ui.take().is_empty(),
            "the factory requests no theme while loading"
        );
    });
}

/// A shutdown without a running wait, delivered any number of times, ends cleanly and changes no
/// theme.
#[test]
fn mac_system_theme_shutdown_without_a_wait_is_a_no_op() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let ui = Dialogs::install(&loaded.runner);
        let event = json!({"type": "session_shutdown", "reason": "quit"});
        for _ in 0..2 {
            assert_eq!(
                emit::<SessionShutdownEvent>(&loaded.runner, &event).await,
                Ok(Value::Null)
            );
        }
        assert!(ui.take().is_empty());
    });
}
