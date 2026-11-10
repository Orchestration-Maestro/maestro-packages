//! The shipped question tool's call and result rows, rendered through its registered renderers.
use super::lane_b::{Instance, render_call, render_result};
use super::support::local;
use serde_json::{Value, json};

/// The variable that names the built question component.
const COMPONENT: &str = "MAESTRO_QUESTION_COMPONENT";

/// Complete and empty calls retain the title and supplied option order.
#[test]
fn question_renders_calls() {
    local(async {
        let instance = Instance::new(COMPONENT).await;
        let runner = instance.runner();
        assert_eq!(
            render_call(
                runner,
                "question",
                &json!({"question":"Which?","options":[{"label":"Rust"},{"label":"Go"}]}),
                200
            ),
            [
                "question Which?",
                "  Options: 1. Rust, 2. Go, 3. Type something."
            ]
        );
        assert_eq!(
            render_call(
                runner,
                "question",
                &json!({"question":"Which?","options":[]}),
                200
            ),
            ["question Which?"]
        );
    });
}

/// Answer kinds and missing details retain their authored display.
#[test]
fn question_renders_results() {
    local(async {
        let instance = Instance::new(COMPONENT).await;
        let runner = instance.runner();
        for (details, expected) in [
            (json!({"answer":null,"options":["Rust","Go"]}), "Cancelled"),
            (
                json!({"answer":"snow","options":["Rust"],"wasCustom":true}),
                "✓ (wrote) snow",
            ),
            (
                json!({"answer":"Rust","options":["Rust","Rust"],"wasCustom":false}),
                "✓ 1. Rust",
            ),
            (
                json!({"answer":"unknown","options":["Rust"],"wasCustom":false}),
                "✓ unknown",
            ),
        ] {
            let result = json!({"content":[],"details":details});
            assert_eq!(
                render_result(runner, "question", &result, false, 200),
                [expected]
            );
        }
    });
}

/// Absent and falsey details show the first text block rather than cancellation.
#[test]
fn question_results_fall_back_to_the_first_text_block() {
    local(async {
        let instance = Instance::new(COMPONENT).await;
        let runner = instance.runner();
        let text = |details: Option<Value>| {
            let mut result = json!({"content":[{"type":"text","text":"fallback"}]});
            if let Some(details) = details {
                result["details"] = details;
            }
            render_result(runner, "question", &result, false, 200)
        };
        assert_eq!(text(None), ["fallback"]);
        for details in [Value::Null, json!(false), json!(0), json!("")] {
            assert_eq!(text(Some(details.clone())), ["fallback"], "{details}");
        }
        assert!(render_result(runner, "question", &json!({"content":[]}), false, 200).is_empty());
    });
}
