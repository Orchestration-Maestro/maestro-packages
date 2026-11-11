//! The shipped question tool loaded into the production engine.
use super::support::{call_tool, harness, load, local};
use serde_json::json;

/// The variable that names the built question component.
const COMPONENT: &str = "MAESTRO_QUESTION_COMPONENT";

/// Without an interactive UI the tool answers with the unavailable-UI error, keeps the option
/// labels and reports a null answer.
#[test]
fn question_headless_preserves_labels_and_null_answer() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let asked = call_tool(
            &loaded.runner,
            "question",
            json!({"question": "Which?", "options": [{"label": "Rust", "description": "Fast"}]}),
        )
        .await;
        assert_eq!(
            asked,
            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": "Error: UI not available (running in non-interactive mode)"
                }],
                "details": {"question": "Which?", "options": ["Rust"], "answer": null}
            }))
        );
    });
}
