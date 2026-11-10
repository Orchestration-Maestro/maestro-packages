//! The shipped structured answer renderer loaded into the production engine.
use super::support::{Shown, harness, load, local, render_result};
use serde_json::{Value, json};

/// The variable that names the built structured-answer component.
const COMPONENT: &str = "MAESTRO_STRUCTURED_OUTPUT_COMPONENT";

/// Both expansion states keep the whole answer; a missing or falsy detail shows only the first
/// text block.
#[test]
fn structured_output_renders_complete_answers_and_fallbacks() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let details =
            json!({"headline":"Headline","summary":"Summary","actionItems":["Review"," Ship 雪 "]});
        let text = json!([{"type":"text","text":"fallback"}]);
        let image_first = json!([{"type":"image","data":"AA==","mimeType":"image/png"},
            {"type":"text","text":"not first"}]);
        for expanded in [false, true] {
            for (content, details, expected) in [
                (
                    json!([]),
                    Some(details.clone()),
                    "Headline\nSummary\n\n1. Review\n2.  Ship 雪",
                ),
                (text.clone(), None, "fallback"),
                (json!([]), Some(json!(null)), ""),
                (text.clone(), Some(json!(false)), "fallback"),
                (text.clone(), Some(json!(0)), "fallback"),
                (text.clone(), Some(json!(-0.0)), "fallback"),
                (text.clone(), Some(json!("")), "fallback"),
                (image_first.clone(), None, ""),
                (
                    json!([]),
                    Some(json!({"headline":"Headline","summary":"Summary","actionItems":[]})),
                    "Headline\nSummary",
                ),
            ] {
                let result = json!({"content": content, "details": details});
                let rows = render_result(
                    &loaded.runner,
                    "structured_output",
                    &Shown {
                        expanded,
                        ..Shown::new(&Value::Null, &result)
                    },
                );
                assert_eq!(
                    rows.join("\n").trim_end(),
                    expected,
                    "{content} {details:?} expanded {expanded}"
                );
            }
        }
    });
}
