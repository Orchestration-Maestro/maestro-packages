//! The shipped prompt-guidance component loaded into the production engine.
use super::support::{before_agent_start, harness, load, local};
use serde_json::Value;

/// The variable that names the built prompt-guidance component.
const COMPONENT: &str = "MAESTRO_PROMPT_CUSTOMIZER_COMPONENT";

/// Recorded events and results consumed by these tests.
fn corpus() -> Value {
    serde_json::from_str(include_str!("prompt_customizer_expected.json")).unwrap()
}

/// Every recorded prompt option set produces its expected guidance.
#[test]
fn prompt_guidance_uses_supplied_tools_and_skills() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        assert!(loaded.runner.has_handlers("before_agent_start"));
        for row in corpus()["rows"].as_array().unwrap() {
            let answer = before_agent_start(&loaded.runner, &row["event"]).await;
            assert_eq!(answer, row["result"], "{}", row["name"]);
        }
    });
}
