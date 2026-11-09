//! The interactive flow: stream a session's output, read prompts, and stop or abort on
//! request.
use maestro_app::{RpcClient, RpcClientError, compact_json, is_whitespace_scalar};
use serde_json::Value;
use std::{cell::RefCell, io::Write, rc::Rc};
use tokio::sync::mpsc::UnboundedReceiver;

/// Where the flow writes.
pub type Output = Rc<RefCell<dyn Write>>;

/// How many characters of a tool result the flow shows.
const PREVIEW_CHARS: usize = 200;

/// Runs the flow until an `exit` line, which stops the client, or an interrupt while no turn
/// runs, which asks the child to terminate without waiting for its end. Lines that arrive during a turn are ignored; an interrupt
/// during a turn sends an abort without waiting for it.
///
/// # Errors
/// Returns the client's failure to start, to read the state or to run a turn, and output
/// failures.
pub async fn run(
    client: Rc<RpcClient>,
    mut input: UnboundedReceiver<String>,
    mut interrupts: UnboundedReceiver<()>,
    output: Output,
) -> Result<(), RpcClientError> {
    let events = Rc::clone(&output);
    let _render = client.on_event(Rc::new(move |event: &Value| {
        Ok(render_event(event, &mut *events.borrow_mut())?)
    }));
    client.start().await?;
    write_state(&client, &output).await?;
    write!(output.borrow_mut(), "You: ")?;
    output.borrow_mut().flush()?;
    let mut turn = None;
    loop {
        tokio::select! {
            biased;
            Some(()) = interrupts.recv() => {
                if turn.is_none() {
                    tokio::task::spawn_local(async move { client.stop().await });
                    // One yield runs the stop and the child's driver, which asks the child to
                    // terminate before the caller tears the task set down.
                    tokio::task::yield_now().await;
                    return Ok(());
                }
                writeln!(output.borrow_mut(), "\n[Aborting...]")?;
                let client = Rc::clone(&client);
                tokio::task::spawn_local(async move { client.abort().await });
            }
            Some(line) = input.recv() => {
                if turn.is_some() {
                    continue;
                }
                if line.trim_matches(is_whitespace_scalar) == "exit" {
                    return client.stop().await;
                }
                let client = Rc::clone(&client);
                turn = Some(Box::pin(async move { client.prompt_and_wait(&line, None, None).await }));
            }
            Some(outcome) = finish(&mut turn) => {
                turn = None;
                outcome?;
                write!(output.borrow_mut(), "\n\nYou: ")?;
                output.borrow_mut().flush()?;
            }
            else => return Ok(()),
        }
    }
}

/// The outcome of the running turn; `None` at once without one.
async fn finish<F: std::future::Future + Unpin>(turn: &mut Option<F>) -> Option<F::Output> {
    match turn {
        Some(turn) => Some(turn.await),
        None => None,
    }
}

/// Writes the selected model and the thinking level, then the greeting.
async fn write_state(client: &RpcClient, output: &Output) -> Result<(), RpcClientError> {
    let state = client.get_state().await?;
    let (provider, id) = state.model.as_ref().map_or_else(
        || ("undefined".to_owned(), "undefined".to_owned()),
        |model| {
            let model = model
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (model.provider.clone(), model.id.clone())
        },
    );
    let level = serde_json::to_value(state.thinking_level)?;
    let mut output = output.borrow_mut();
    writeln!(output, "Model: {provider}/{id}")?;
    writeln!(output, "Thinking: {}\n", level.as_str().unwrap_or("off"))?;
    writeln!(output, "Interactive RPC example. Type 'exit' to quit.\n")?;
    Ok(())
}

/// Writes a text or thinking delta, a tool's name, or the start of a tool's result.
fn render_event(event: &Value, output: &mut dyn Write) -> std::io::Result<()> {
    match event["type"].as_str() {
        Some("message_update") => {
            let update = &event["assistantMessageEvent"];
            if matches!(
                update["type"].as_str(),
                Some("text_delta" | "thinking_delta")
            ) && let Some(delta) = update["delta"].as_str()
            {
                output.write_all(delta.as_bytes())?;
            }
            output.flush()
        }
        Some("tool_execution_start") => {
            let name = event["toolName"].as_str().unwrap_or("undefined");
            writeln!(output, "\n[Tool: {name}]")
        }
        Some("tool_execution_end") => {
            writeln!(output, "[Result: {}...]\n", preview(&event["result"])?)
        }
        _ => Ok(()),
    }
}

/// The first characters of `result` written as JSON.
pub fn preview(result: &Value) -> Result<String, serde_json::Error> {
    Ok(compact_json(result)?.chars().take(PREVIEW_CHARS).collect())
}

/// Writes a failure of the flow to `stderr`.
pub fn report(result: Result<(), RpcClientError>, stderr: &mut dyn Write) {
    if let Err(error) = result {
        let _ = writeln!(stderr, "{error}");
    }
}
