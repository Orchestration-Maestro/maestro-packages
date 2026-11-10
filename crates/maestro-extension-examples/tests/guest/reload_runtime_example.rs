//! The shipped reload command and follow-up tool loaded into the production engine.
use super::support::{harness, load, local, only_tool_metadata, result_value};
use maestro_extensions_wasmtime::test_support::{
    Harness, Loaded,
    maestro_extensions::{Cancellation, ExtensionCommandContextActions, wrap_registered_tool},
    unconfigured_command_operations,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};
use tokio::sync::{mpsc, oneshot};

/// The variable that names the built reload component.
const COMPONENT: &str = "MAESTRO_RELOAD_RUNTIME_COMPONENT";

/// The recorded registration and observation rows these tests consume.
fn corpus() -> Value {
    serde_json::from_str(include_str!("reload_runtime_expected.json")).unwrap()
}

/// What the host was asked to do, in order.
type Calls = Rc<RefCell<Vec<Value>>>;

/// One loaded component whose reload and follow-up delivery the test controls.
struct Reloading {
    /// The engine the component lives in.
    harness: Harness,
    /// The runner over the component.
    loaded: Loaded,
    /// Every host operation the component requested.
    calls: Calls,
}

impl Reloading {
    /// Loads the component; `reload` serves the reload operation, `deliver` each follow-up.
    async fn new(
        reload: impl Fn() -> std::pin::Pin<Box<dyn Future<Output = Result<(), String>>>> + 'static,
        deliver_error: Option<String>,
    ) -> Self {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let calls: Calls = Rc::default();
        let recorded = calls.clone();
        let mut operations = unconfigured_command_operations();
        operations.reload = Rc::new(move || {
            recorded.borrow_mut().push(json!({"operation": "reload"}));
            let finished = reload();
            Box::pin(async move { finished.await.map_err(Into::into) })
        });
        loaded
            .runner
            .bind_command_context(Some(ExtensionCommandContextActions::new(operations)));
        let sent = calls.clone();
        harness.runtime.actions.write().unwrap().send_user_message =
            Rc::new(move |content, options| {
                if let Some(error) = &deliver_error {
                    return Err(error.clone().into());
                }
                sent.borrow_mut().push(json!({
                    "operation": "sendUserMessage",
                    "content": content,
                    "options": options,
                }));
                Ok(())
            });
        Self {
            harness,
            loaded,
            calls,
        }
    }

    /// A component whose reload settles at once with `error`, if any.
    async fn immediate(error: Option<String>, deliver_error: Option<String>) -> Self {
        Self::new(
            move || {
                let error = error.clone();
                Box::pin(async move { error.map_or(Ok(()), Err) })
            },
            deliver_error,
        )
        .await
    }

    /// Runs the reload command with `args`.
    async fn command(&self, args: &str) -> Result<(), String> {
        let command = self
            .loaded
            .runner
            .get_command("reload-runtime")
            .unwrap()
            .command;
        (command.handler)(args.into(), self.loaded.runner.create_command_context())
            .await
            .map_err(|error| error.to_string())
    }

    /// Calls the follow-up tool and returns each outcome.
    async fn tool(&self, input: &Value) -> Vec<Result<Value, String>> {
        let runner = &self.loaded.runner;
        let tools = runner.get_all_registered_tools();
        let tool = wrap_registered_tool(&tools[0], runner.clone());
        let mut outcomes = Vec::new();
        for _ in 0..input["repetitions"].as_u64().unwrap() {
            let signal = input["aborted"].as_bool().unwrap().then(|| {
                let signal = Cancellation::new();
                signal.abort();
                signal
            });
            let outcome =
                (tool.execute)("call".into(), input["params"].clone(), signal, None).await;
            outcomes.push(
                outcome
                    .map(|result| result_value(&result))
                    .map_err(|error| error.to_string()),
            );
        }
        outcomes
    }
}

/// The command and tool register with the recorded metadata.
#[test]
fn reload_runtime_registers_command_and_tool() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let recorded = &corpus()["metadata"];
        let commands = loaded.runner.get_registered_commands();
        assert_eq!(commands.len(), 1);
        let command = &commands[0].command;
        assert_eq!(
            json!({
                "name": command.name,
                "description": command.description,
                "completions": command.get_argument_completions.is_some(),
            }),
            recorded["command"]
        );
        let tool = only_tool_metadata(&loaded.runner);
        for field in ["name", "label", "description", "parameters"] {
            assert_eq!(tool[field], recorded["tool"][field], "{field}");
        }
    });
}

/// The command waits for the host's reload and ignores its arguments.
#[test]
fn reload_runtime_command_awaits_reload_and_returns() {
    local(async {
        for row in corpus()["commands"].as_array().unwrap() {
            if row["expected"]["outcome"] != "fulfilled" {
                continue;
            }
            let args = row["input"]["args"].as_str().unwrap();
            if row["input"]["hold"] == true {
                held(args, &row["expected"]).await;
                continue;
            }
            let reloading = Reloading::immediate(None, None).await;
            assert_eq!(reloading.command(args).await, Ok(()), "{}", row["name"]);
            assert_eq!(
                Value::Array(reloading.calls.borrow().clone()),
                row["expected"]["calls"]
            );
            drop(reloading.harness);
        }
    });
}

/// A reload the host has not finished keeps the command pending until it is released.
async fn held(args: &str, expected: &Value) {
    let (entered, mut began) = mpsc::unbounded_channel();
    let (release, released) = oneshot::channel::<()>();
    let released = Rc::new(RefCell::new(Some(released)));
    let reloading = Reloading::new(
        move || {
            entered.send(()).unwrap();
            let released = released.borrow_mut().take().unwrap();
            Box::pin(async move {
                released.await.unwrap();
                Ok(())
            })
        },
        None,
    )
    .await;
    let command = reloading
        .loaded
        .runner
        .get_command("reload-runtime")
        .unwrap()
        .command;
    let running = tokio::task::spawn_local((command.handler)(
        args.into(),
        reloading.loaded.runner.create_command_context(),
    ));
    began.recv().await.unwrap();
    assert!(
        !running.is_finished(),
        "pending before the host releases it"
    );
    assert_eq!(expected["pendingBeforeRelease"], true);
    release.send(()).unwrap();
    assert!(running.await.unwrap().is_ok());
    assert_eq!(
        Value::Array(reloading.calls.borrow().clone()),
        expected["calls"]
    );
}

/// A reload the host refuses is the command's own failure.
#[test]
fn reload_runtime_command_propagates_reload_failure() {
    local(async {
        for row in corpus()["commands"].as_array().unwrap() {
            if row["expected"]["outcome"] != "rejected" {
                continue;
            }
            let error = row["input"]["error"].as_str().unwrap().to_owned();
            let reloading = Reloading::immediate(Some(error), None).await;
            let args = row["input"]["args"].as_str().unwrap();
            assert_eq!(
                reloading.command(args).await.err().as_deref(),
                row["expected"]["error"].as_str()
            );
            assert_eq!(
                Value::Array(reloading.calls.borrow().clone()),
                row["expected"]["calls"]
            );
        }
    });
}

/// The tool queues the reload as a follow-up before it returns, however it is called.
#[test]
fn reload_runtime_tool_queues_follow_up_before_result() {
    local(async {
        for row in corpus()["tools"].as_array().unwrap() {
            if row["expected"]["outcome"] != "fulfilled" {
                continue;
            }
            let reloading = Reloading::immediate(None, None).await;
            let outcomes = reloading.tool(&row["input"]).await;
            let results: Vec<Value> = outcomes.into_iter().map(Result::unwrap).collect();
            assert_eq!(
                Value::Array(results),
                row["expected"]["results"],
                "{}",
                row["name"]
            );
            assert_eq!(
                Value::Array(reloading.calls.borrow().clone()),
                row["expected"]["calls"]
            );
        }
    });
}

/// A follow-up the host refuses fails the tool call instead of reporting it queued.
#[test]
fn reload_runtime_tool_propagates_delivery_failure() {
    local(async {
        for row in corpus()["tools"].as_array().unwrap() {
            if row["expected"]["outcome"] != "rejected" {
                continue;
            }
            let error = row["input"]["error"].as_str().unwrap().to_owned();
            let reloading = Reloading::immediate(None, Some(error)).await;
            let outcomes = reloading.tool(&row["input"]).await;
            assert_eq!(
                outcomes[0].as_ref().err().map(String::as_str),
                row["expected"]["error"].as_str()
            );
            assert_eq!(
                Value::Array(reloading.calls.borrow().clone()),
                row["expected"]["calls"]
            );
        }
    });
}
