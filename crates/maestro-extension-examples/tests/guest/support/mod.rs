//! Loading a prepared example component into the production engine and calling what it
//! registered.
use maestro_extensions_wasmtime::test_support::maestro_extensions::{
    AgentToolResult, ExtensionRunner, Local, wrap_registered_tool,
};
use maestro_extensions_wasmtime::test_support::{Harness, HarnessOptions, Loaded};
use serde_json::{Value, json};
use std::rc::Rc;

/// Runs a test body on the local task set that loaded components need.
pub fn local<T>(test: impl Future<Output = T>) -> T {
    maestro_extensions_wasmtime::test_support::local(test).unwrap()
}

/// A harness whose engine reuses the code the run compiled.
pub fn harness() -> Harness {
    let options = HarnessOptions {
        agent_dir: maestro_test_process::run_agent_directory().unwrap_or_default(),
        ..HarnessOptions::default()
    };
    Harness::new("/work", options).unwrap()
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

/// The caller-visible content and details of one tool result.
pub fn result_value(result: &AgentToolResult<Value>) -> Value {
    json!({"content": result.content, "details": result.details})
}

/// The declaration of the only tool the runner holds.
pub fn only_tool_metadata(runner: &ExtensionRunner<Local>) -> Value {
    let tools = runner.get_all_registered_tools();
    assert_eq!(tools.len(), 1);
    let metadata = tools[0].metadata();
    json!({
        "name": metadata.definition.name,
        "label": metadata.label,
        "description": metadata.definition.description,
        "parameters": serde_json::to_value(&metadata.definition.parameters).unwrap(),
    })
}

/// Calls the registered tool `name` as an application does and returns its outcome.
pub async fn call_tool(
    runner: &Rc<ExtensionRunner<Local>>,
    name: &str,
    input: Value,
) -> Result<Value, String> {
    let tools = runner.get_all_registered_tools();
    let registered = tools
        .iter()
        .find(|tool| tool.metadata().definition.name == name)
        .unwrap_or_else(|| panic!("no tool named {name}"));
    let tool = wrap_registered_tool(registered, runner.clone());
    (tool.execute)("call".into(), input, None, None)
        .await
        .map(|result| result_value(&result))
        .map_err(|error| error.to_string())
}
