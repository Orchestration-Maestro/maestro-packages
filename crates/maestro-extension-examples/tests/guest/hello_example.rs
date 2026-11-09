//! The shipped greeting component loaded into the production engine.
use super::support::{call_tool, harness, load, local, only_tool_metadata, result_value};
use maestro_extensions_wasmtime::test_support::maestro_extensions::{
    AgentToolUpdateCallback, Cancellation, Local, wrap_registered_tool,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// The variable that names the built greeting component.
const COMPONENT: &str = "MAESTRO_HELLO_COMPONENT";

/// Recorded arguments, metadata and results the greeting is checked against.
fn corpus() -> Value {
    serde_json::from_str(include_str!("hello_expected.json")).unwrap()
}

/// One tool registers with the recorded schema and no argument preparation.
#[test]
fn hello_registers_the_greeting_schema() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        assert_eq!(only_tool_metadata(&loaded.runner), corpus()["metadata"]);
        let tools = loaded.runner.get_all_registered_tools();
        let tool = wrap_registered_tool(&tools[0], loaded.runner);
        assert!(tool.prepare_arguments.is_none());
    });
}

/// Every supplied string, including empty and whitespace-only names, reaches both outputs.
#[test]
fn hello_preserves_supplied_names() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        for row in corpus()["rows"].as_array().unwrap() {
            let result = call_tool(&loaded.runner, "hello", row["input"].clone()).await;
            assert_eq!(result, Ok(row["result"].clone()), "{}", row["input"]);
        }
    });
}

/// An already-aborted signal and a progress callback do not change this tool's answer.
#[test]
fn hello_ignores_unused_invocation_handles() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let tools = loaded.runner.get_all_registered_tools();
        let tool = wrap_registered_tool(&tools[0], loaded.runner.clone());
        let signal = Cancellation::new();
        signal.abort();
        let updates = Rc::new(RefCell::new(Vec::new()));
        let observed = updates.clone();
        let update: AgentToolUpdateCallback<Value, Local> =
            Rc::new(move |partial| observed.borrow_mut().push(result_value(&partial)));
        let result = (tool.execute)(
            "another-call".into(),
            json!({"name": "Ada"}),
            Some(signal),
            Some(update),
        )
        .await
        .unwrap();
        assert_eq!(result_value(&result), corpus()["rows"][0]["result"]);
        assert!(updates.borrow().is_empty());
    });
}

/// A runtime that a reload replaced refuses the greeting's registration, and the refusal
/// reaches the loader as the component's load failure.
#[test]
fn hello_registration_refused_by_a_replaced_runtime_fails_the_load() {
    local(async {
        let harness = harness();
        harness
            .runtime
            .invalidate(Some("replaced by a reload".into()));
        let path = super::support::component(COMPONENT);
        let loaded = harness.load(vec![path.clone()]).await.unwrap();
        let errors: Vec<_> = loaded
            .errors
            .iter()
            .map(|error| (error.path.clone(), error.error.clone()))
            .collect();
        assert_eq!(
            errors,
            [(
                path,
                "Failed to load extension: replaced by a reload".to_owned()
            )]
        );
        assert!(loaded.runner.get_all_registered_tools().is_empty());
    });
}
