//! Loading a prepared example component into the production engine and calling what it
//! registered.
mod events;
mod failures;
mod footer;
mod instance;
mod native;
mod recorder;
mod rows;
mod runtime;
mod session;
mod tools;

use maestro_extensions_wasmtime::test_support::{Harness, HarnessOptions, Loaded};

pub use events::{before_agent_start, emit, event, tool_call, verdict};
pub use failures::Failures;
pub use footer::Metadata;
pub use instance::Instance;
pub use native::{load_native, local_native};
pub use recorder::{Recorder, TERMINAL, drain};
pub use rows::{Shown, dark, plain_rows, plain_text, render_call, render_message, render_result};
pub use runtime::{driven, local, stepped, timed};
pub use session::{
    Bindings, Log, bind, fail_with, register_model, replace_branch, tool_result_message,
};
pub use tools::{call_tool, only_tool_metadata, result_value, run_tool};

/// A harness whose engine reuses the code the run compiled.
pub fn harness() -> Harness {
    harness_in("/work")
}

/// A harness whose session reports `cwd`, with the code cache the run compiled.
pub fn harness_in(cwd: &str) -> Harness {
    let options = HarnessOptions {
        agent_dir: maestro_test_process::run_agent_directory().unwrap_or_default(),
        ..HarnessOptions::default()
    };
    Harness::new(cwd, options).unwrap()
}

/// The built component the setup variable `variable` names.
pub fn component(variable: &str) -> String {
    std::env::var(variable).unwrap_or_else(|_| {
        panic!("{variable} is unset: run just prepare-tests, then cargo nextest")
    })
}

/// Loads the component `variable` names into `harness` and checks that it loaded.
pub async fn load(harness: &Harness, variable: &str) -> Loaded {
    let loaded = harness.load(vec![component(variable)]).await.unwrap();
    let errors: Vec<_> = loaded.errors.iter().map(|error| &error.error).collect();
    assert!(errors.is_empty(), "{errors:?}");
    loaded
}
