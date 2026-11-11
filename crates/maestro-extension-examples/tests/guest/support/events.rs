//! Delivering recorded event documents to a runner as the application does.
use maestro_extensions::{
    ExtensionRunner, Local, Presence, ToolCallEvent, execution::RunnerEmitEvent,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

/// Builds an event record from the document an application delivers.
pub fn event<T: DeserializeOwned>(document: Value) -> T {
    serde_json::from_value(document).unwrap()
}

/// Delivers an event written as JSON to the runner's handlers and returns their answer, or the
/// failure that stopped the delivery.
///
/// The `type` member a recorded event carries only names the event and is not part of its body.
pub async fn try_emit<V: RunnerEmitEvent + DeserializeOwned>(
    runner: &ExtensionRunner<Local>,
    event: &Value,
) -> Result<Option<V::Result>, String> {
    let mut body = event.clone();
    if let Some(object) = body.as_object_mut() {
        object.shift_remove("type");
    }
    let mut event: V = serde_json::from_value(body).map_err(|error| error.to_string())?;
    runner
        .emit(&mut event)
        .await
        .map_err(|error| error.to_string())
}

/// Delivers an event written as JSON to the runner's handlers and returns their answer.
pub async fn emit<V: RunnerEmitEvent + DeserializeOwned>(
    runner: &ExtensionRunner<Local>,
    event: &Value,
) -> Option<V::Result> {
    try_emit::<V>(runner, event).await.unwrap()
}

/// Delivers an event and returns the verdict the handlers gave as JSON, or null when none did.
pub async fn verdict<V>(runner: &ExtensionRunner<Local>, event: &Value) -> Result<Value, String>
where
    V: RunnerEmitEvent + DeserializeOwned,
    V::Result: Serialize,
{
    let verdict = try_emit::<V>(runner, event).await?;
    Ok(verdict.map_or(Value::Null, |verdict| json!(verdict)))
}

/// Delivers a recorded tool call to the hooks and returns the verdict, or null when none was given.
pub async fn tool_call(runner: &ExtensionRunner<Local>, event: &Value) -> Result<Value, String> {
    let mut event: ToolCallEvent =
        serde_json::from_value(event.clone()).map_err(|error| error.to_string())?;
    let verdict = runner
        .emit_tool_call(&mut event)
        .await
        .map_err(|error| error.to_string())?;
    Ok(verdict.map_or(Value::Null, |verdict| json!(verdict)))
}

/// Delivers a recorded `before_agent_start` event document to the runner as the application does
/// and returns what the handlers answered: the replacement prompt, or null when none changed it.
pub async fn before_agent_start(runner: &ExtensionRunner<Local>, event: &Value) -> Value {
    let options = serde_json::from_value(event["systemPromptOptions"].clone()).unwrap();
    let mut images = Presence::Missing;
    let combined = runner
        .emit_before_agent_start(
            event["prompt"].as_str().unwrap().to_owned(),
            &mut images,
            event["systemPrompt"].as_str().unwrap().to_owned(),
            options,
        )
        .await
        .unwrap();
    let Some(combined) = combined else {
        return Value::Null;
    };
    assert!(combined.messages.is_none(), "no custom message is expected");
    json!({"systemPrompt": combined.system_prompt})
}
