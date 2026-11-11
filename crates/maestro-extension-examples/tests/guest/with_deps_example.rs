//! The shipped duration component, which carries its own parsing dependencies, loaded into the
//! production engine.
use super::support::{call_tool, harness, load, local, only_tool_metadata, result_value};
use maestro_extensions_wasmtime::test_support::maestro_extensions::{
    AgentToolUpdateCallback, Cancellation, ExtensionRunner, Local, wrap_registered_tool,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// The variable that names the built duration component.
const COMPONENT: &str = "MAESTRO_WITH_DEPS_COMPONENT";

/// Recorded metadata and outcomes consumed by these tests.
fn corpus() -> Value {
    serde_json::from_str(include_str!("with_deps_expected.json")).unwrap()
}

/// The recorded rows of one group.
fn rows(group: &str) -> Vec<Value> {
    corpus()["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["group"] == group)
        .cloned()
        .collect()
}

/// The arguments of one recorded row, with or without an unread additional member.
fn params(row: &Value, extra: bool) -> Value {
    let mut params = json!({"duration": row["duration"]});
    if extra {
        params["note"] = json!("ignored");
    }
    params
}

/// Compare one recorded row's complete outcome with a caller-visible one.
fn assert_outcome(row: &Value, actual: &Result<Value, String>) {
    let input = row["duration"].as_str().unwrap();
    let expected = match row["outcome"].as_str() {
        Some("ok") => Ok(row["result"].clone()),
        _ => Err(row["error"].as_str().unwrap().to_owned()),
    };
    assert_eq!(actual, &expected, "input {input:?}");
}

/// Loads the component and runs the recorded rows of one group, checking the group's size.
fn run_group(group: &str, count: usize) {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let recorded = rows(group);
        assert_eq!(recorded.len(), count, "{group} rows");
        for row in recorded {
            let actual = call_tool(&loaded.runner, "parse_duration", params(&row, false)).await;
            assert_outcome(&row, &actual);
        }
    });
}

/// Register once and assert the declared required string without optional metadata.
#[test]
fn with_deps_registers_duration_schema() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        assert_eq!(only_tool_metadata(&loaded.runner), corpus()["metadata"]);
        let tools = loaded.runner.get_all_registered_tools();
        let tool = wrap_registered_tool(&tools[0], loaded.runner);
        assert!(tool.prepare_arguments.is_none());
    });
}

/// The authored examples and every alias keep their exact number and text.
#[test]
fn with_deps_preserves_unit_results() {
    run_group("authored", 3);
    run_group("unit_aliases", 31);
}

/// Every boundary row keeps its exact rendered number or its rejection.
#[test]
fn with_deps_preserves_numeric_boundaries() {
    run_group("numeric_boundaries", 20);
}

/// Unsupported text keeps the literal input in its own error message.
#[test]
fn with_deps_rejects_invalid_duration_text() {
    run_group("rejection_or_native_extension", 33);
}

/// Calls the duration tool with the supplied handles.
async fn call_with_handles(
    runner: &Rc<ExtensionRunner<Local>>,
    params: Value,
    signal: Cancellation,
    update: AgentToolUpdateCallback<Value, Local>,
) -> Value {
    let tools = runner.get_all_registered_tools();
    let tool = wrap_registered_tool(&tools[0], runner.clone());
    let result = (tool.execute)("another-call".into(), params, Some(signal), Some(update))
        .await
        .unwrap();
    result_value(&result)
}

/// Unread invocation handles and an additional member change nothing, and a call after a
/// rejected one still succeeds.
#[test]
fn with_deps_ignores_unused_invocation_handles() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let signal = Cancellation::new();
        signal.abort();
        let updates = Rc::new(RefCell::new(Vec::new()));
        let observed = updates.clone();
        let update: AgentToolUpdateCallback<Value, Local> =
            Rc::new(move |partial| observed.borrow_mut().push(result_value(&partial)));
        let row = rows("unused_member").pop().unwrap();
        let actual = call_with_handles(&loaded.runner, params(&row, true), signal, update).await;
        assert_eq!(actual, row["result"]);
        assert!(updates.borrow().is_empty());
        let rejected = rows("rejection_or_native_extension").pop().unwrap();
        let actual = call_tool(&loaded.runner, "parse_duration", params(&rejected, false)).await;
        assert_outcome(&rejected, &actual);
        let actual = call_tool(&loaded.runner, "parse_duration", params(&row, false)).await;
        assert_outcome(&row, &actual);
    });
}

/// Every recorded row of the built single-copy example registers and executes across the
/// component interface.
#[test]
fn with_deps_component_uses_its_parsing_dependency() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        for row in corpus()["rows"].as_array().unwrap() {
            let actual = call_tool(&loaded.runner, "parse_duration", params(row, false)).await;
            assert_outcome(row, &actual);
        }
    });
}
