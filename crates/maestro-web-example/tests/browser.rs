//! Real-browser runs of the chat example's exports in the pinned Chromium.
//!
//! `just browser-test` compiles `program/` to WebAssembly, binds it into the directory named by
//! `MAESTRO_BROWSER_EXPORTS` and runs these tests with the browser named by `CHROMIUM_PATH`.

use maestro_test_browser::{browser_driver, test_support};
use maestro_test_directory::Scratch;
use std::path::PathBuf;

/// The page family the example's exports run in.
const FAMILY: &str = "contributions";

/// A location the browser recipe supplies.
fn supplied(variable: &str) -> PathBuf {
    std::env::var_os(variable)
        .unwrap_or_else(|| panic!("the browser recipe supplies {variable}"))
        .into()
}

/// Run one export of the prepared program in its own browser.
fn export(name: &str) {
    let profile = Scratch::new().unwrap();
    test_support::runtime()
        .unwrap()
        .block_on(browser_driver::run_export(
            &supplied("CHROMIUM_PATH"),
            profile.path(),
            &supplied("MAESTRO_BROWSER_EXPORTS"),
            FAMILY,
            name,
        ))
        .unwrap();
}

/// Declare one browser test per export, so the runner bounds and names each of them.
macro_rules! exports {
    ($($name:ident),+ $(,)?) => {
        /// The export names, in the order the program declares them.
        const EXPORTS: &[&str] = &[$(stringify!($name)),+];
        $(
            #[test]
            #[ignore = "runs in the pinned Chromium job"]
            fn $name() {
                export(stringify!($name));
            }
        )+
    };
}

exports!(
    chat_example_titles_and_admission_follow_the_first_user_message,
    chat_example_saves_and_restores_sessions_from_the_url,
    chat_example_edits_titles_and_follows_new_and_deleted_sessions,
    chat_example_prompts_notifies_and_opens_settings,
    chat_example_message_completion_keeps_title_drafts,
    chat_example_title_saves_stay_with_the_originating_session,
    chat_example_new_session_aborts_the_outgoing_request,
);

/// Every export the program declares has a browser test, and no test names a missing export.
#[test]
fn every_program_export_has_a_browser_test() {
    let source = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/program/chat_example_browser.rs"),
    )
    .unwrap();
    let lines: Vec<_> = source.lines().map(str::trim).collect();
    let declared: Vec<_> = lines
        .windows(2)
        .filter(|pair| pair[0] == "#[wasm_bindgen]")
        .filter_map(|pair| pair[1].strip_prefix("pub "))
        .map(|line| line.strip_prefix("async ").unwrap_or(line))
        .filter_map(|line| line.strip_prefix("fn ")?.split_once('('))
        .map(|(name, _)| name)
        .collect();
    assert_eq!(declared, EXPORTS);
}
