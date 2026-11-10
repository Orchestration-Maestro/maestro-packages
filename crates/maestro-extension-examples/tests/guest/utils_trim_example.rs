//! The shared trimming helper's boundaries, observed through the shipped input hook that calls it.
use super::support::{Recorder, harness, load, local};
use maestro_extensions_wasmtime::test_support::maestro_extensions::{InputSource, Presence};
use serde_json::json;

/// The variable that names the built input component, which trims its quick questions.
const COMPONENT: &str = "MAESTRO_INPUT_TRANSFORM_COMPONENT";

/// Trimming removes the host's whitespace from both ends, keeps interior whitespace and treats
/// the next-line control character as text.
#[test]
fn trim_uses_host_boundaries() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let ui = Recorder::install(&loaded.runner);
        for (text, trimmed) in [
            ("", ""),
            (" \t\u{feff}\n", ""),
            (" \u{feff}hé \tllo\n", "hé \tllo"),
            (" \u{85}text\u{85} ", "\u{85}text\u{85}"),
        ] {
            let mut images = Presence::Missing;
            let decision = loaded
                .runner
                .emit_input(
                    &format!("?quick {text}"),
                    &mut images,
                    InputSource::Interactive,
                    None,
                )
                .await
                .unwrap();
            let expected = if trimmed.is_empty() {
                assert_eq!(
                    ui.take()[0][1],
                    json!(["Usage: ?quick <question>", "Warning"])
                );
                json!({"action": "handled"})
            } else {
                json!({
                    "action": "transform",
                    "text": format!("Respond briefly in 1-2 sentences: {trimmed}")
                })
            };
            assert_eq!(
                serde_json::to_value(decision).unwrap(),
                expected,
                "{text:?}"
            );
        }
    });
}
