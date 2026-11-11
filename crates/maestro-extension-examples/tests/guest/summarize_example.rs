//! The shipped summary command's early exits, loaded into the production engine.
use super::support::{Recorder, TERMINAL, harness, load, local};
use maestro_extensions_wasmtime::test_support::{
    Harness, Loaded, maestro_extensions::SessionManager,
};
use serde_json::json;

/// The variable that names the built summary component.
const COMPONENT: &str = "MAESTRO_SUMMARIZE_COMPONENT";

/// Puts one user message with `text` on the session's current branch.
fn say(harness: &Harness, text: &str) {
    let mut history = SessionManager::in_memory(Some("/work")).unwrap();
    history
        .append_message(json!({"role":"user","content":text,"timestamp":1}))
        .unwrap();
    *harness.session_manager.borrow_mut() = history;
}

/// Runs the registered command once and returns its failure, if any.
async fn summarize(loaded: &Loaded) -> Option<String> {
    let command = loaded.runner.get_command("summarize").unwrap().command;
    (command.handler)("ignored".into(), loaded.runner.create_command_context())
        .await
        .err()
        .map(|error| error.to_string())
}

/// A conversation without text reports that and asks the catalog nothing; headless it is
/// silent.
#[test]
fn summarize_no_text_skips_catalog_and_completion() {
    local(async {
        for text in ["", "\u{feff}\u{a0} "] {
            let harness = harness();
            let loaded = load(&harness, COMPONENT).await;
            let ui = Recorder::install_on(&loaded.runner, TERMINAL);
            say(&harness, text);
            assert_eq!(summarize(&loaded).await, None);
            assert_eq!(
                ui.take(),
                [json!(["notify", ["No conversation text found", "Warning"]])]
            );
            ui.connect(&loaded.runner, false);
            assert_eq!(summarize(&loaded).await, None);
            assert!(ui.take().is_empty());
        }
    });
}

/// A model without a key is reported after the preparing notice; headless it is silent.
#[test]
fn summarize_reports_a_missing_key() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let ui = Recorder::install_on(&loaded.runner, TERMINAL);
        say(&harness, "hello");
        assert_eq!(summarize(&loaded).await, None);
        assert_eq!(
            ui.take(),
            [
                json!(["notify", ["Preparing summary...", "Info"]]),
                json!(["notify", ["No API key for openai/gpt-5.2", "Warning"]])
            ]
        );
        ui.connect(&loaded.runner, false);
        assert_eq!(summarize(&loaded).await, None);
        assert!(ui.take().is_empty());
    });
}
