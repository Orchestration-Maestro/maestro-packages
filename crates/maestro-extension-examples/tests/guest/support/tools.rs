//! Calling the tools an extension registered, as the application does.
use maestro_extensions::{
    AgentToolResult, AgentToolUpdateCallback, Cancellation, ExtensionRunner, Local,
    wrap_registered_tool,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

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

/// What one tool call produced: its outcome and the progress updates it delivered.
pub struct Called {
    /// The result the caller sees, or the failure message.
    pub outcome: Result<Value, String>,
    /// Each progress update, as the caller sees it.
    pub updates: Vec<Value>,
}

/// Calls the registered tool `name` as an application does, with an optional cancellation and a
/// progress callback that runs `on_update` after it records each update.
pub async fn run_tool(
    runner: &Rc<ExtensionRunner<Local>>,
    name: &str,
    input: Value,
    signal: Option<Cancellation>,
    on_update: Option<Rc<dyn Fn()>>,
) -> Called {
    let tools = runner.get_all_registered_tools();
    let registered = tools
        .iter()
        .find(|tool| tool.metadata().definition.name == name)
        .unwrap_or_else(|| panic!("no tool named {name}"));
    let tool = wrap_registered_tool(registered, runner.clone());
    let updates = Rc::new(RefCell::new(Vec::new()));
    let sink = updates.clone();
    let update: AgentToolUpdateCallback<Value, Local> = Rc::new(move |partial| {
        sink.borrow_mut().push(result_value(&partial));
        if let Some(on_update) = &on_update {
            on_update();
        }
    });
    let outcome = (tool.execute)("call".into(), input, signal, Some(update))
        .await
        .map(|result| result_value(&result))
        .map_err(|error| error.to_string());
    Called {
        outcome,
        updates: updates.take(),
    }
}

/// Calls the registered tool `name` as an application does and returns its outcome.
pub async fn call_tool(
    runner: &Rc<ExtensionRunner<Local>>,
    name: &str,
    input: Value,
) -> Result<Value, String> {
    run_tool(runner, name, input, None, None).await.outcome
}
