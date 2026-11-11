//! The shipped shutdown component loaded into the production engine.
use super::support::{Bindings, Instance, call_tool, local, run_tool};
use maestro_extensions_wasmtime::test_support::maestro_extensions::{
    Cancellation, ExtensionRunner, Local,
};
use serde_json::{Value, json};
use std::{cell::Cell, rc::Rc};

/// The variable that names the built shutdown component.
const COMPONENT: &str = "MAESTRO_SHUTDOWN_COMMAND_COMPONENT";

/// Recorded metadata, arguments and results these tests consume.
fn corpus() -> Value {
    serde_json::from_str(include_str!("shutdown_expected.json")).unwrap()
}

/// One loaded component whose shutdown requests are counted, or refused with `refusal`.
struct Session {
    /// The loaded component and its engine.
    instance: Instance,
    /// How many shutdowns the application was asked for.
    requests: Rc<Cell<usize>>,
}

impl Session {
    /// Loads the component with a shutdown operation that counts and, when `refusal` is set,
    /// fails with it.
    async fn new(refusal: Option<&str>) -> Self {
        let instance = Instance::new(COMPONENT).await;
        let requests = Rc::new(Cell::new(0));
        let counted = requests.clone();
        let refusal = refusal.map(str::to_owned);
        let mut bindings = Bindings::new();
        bindings.context.shutdown = Rc::new(move || {
            if let Some(message) = &refusal {
                return Err(message.clone().into());
            }
            counted.set(counted.get() + 1);
            Ok(())
        });
        bindings.apply(instance.runner());
        Self { instance, requests }
    }

    /// The runner over the loaded component.
    fn runner(&self) -> &Rc<ExtensionRunner<Local>> {
        self.instance.runner()
    }

    /// Runs the `quit` command with `args`.
    async fn quit(&self, args: &str) -> Result<(), String> {
        let command = self.runner().get_command("quit").unwrap().command;
        (command.handler)(args.into(), self.runner().create_command_context())
            .await
            .map_err(|error| error.to_string())
    }
}

/// The declaration of every registered tool, in registration order.
fn tool_declarations(runner: &ExtensionRunner<Local>) -> Value {
    let declared: Vec<Value> = runner
        .get_all_registered_tools()
        .iter()
        .map(|tool| {
            let metadata = tool.metadata();
            json!({
                "name": metadata.definition.name,
                "label": metadata.label,
                "description": metadata.definition.description,
                "parameters": serde_json::to_value(&metadata.definition.parameters).unwrap(),
            })
        })
        .collect();
    json!(declared)
}

/// The details a completed deployment reports for `environment`.
fn deployed(environment: &Value) -> Value {
    json!({
        "content": [{"type": "text", "text": corpus()["deploymentResult"]}],
        "details": {"environment": environment}
    })
}

/// The command and both tools register with the recorded declarations.
#[test]
fn shutdown_example_registers_command_and_tool_contracts() {
    local(async {
        let session = Session::new(None).await;
        let runner = session.runner();
        let commands = runner.get_registered_commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].command.name, "quit");
        assert_eq!(
            json!(commands[0].command.description),
            "Exit Maestro cleanly"
        );
        assert!(commands[0].command.get_argument_completions.is_none());
        assert_eq!(tool_declarations(runner), corpus()["tools"]);
        for tool in runner.get_all_registered_tools() {
            let record =
                maestro_extensions_wasmtime::test_support::maestro_extensions::wrap_registered_tool(
                    &tool,
                    runner.clone(),
                );
            assert!(record.prepare_arguments.is_none());
        }
    });
}

/// Each supplied argument text reaches the command, which requests one shutdown.
#[test]
fn quit_command_requests_shutdown() {
    local(async {
        let session = Session::new(None).await;
        for (index, args) in corpus()["commandArguments"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            session.quit(args.as_str().unwrap()).await.unwrap();
            assert_eq!(session.requests.get(), index + 1);
        }
    });
}

/// The finishing tool requests shutdown before it hands back its exact result.
#[test]
fn finish_tool_returns_after_request() {
    local(async {
        let session = Session::new(None).await;
        let expected = corpus();
        for (index, row) in expected["finish"].as_array().unwrap().iter().enumerate() {
            let signal = Cancellation::new();
            if row["aborted"] == true {
                signal.abort();
            }
            let called = run_tool(
                session.runner(),
                "finish_and_exit",
                row["params"].clone(),
                Some(signal),
                None,
            )
            .await;
            assert_eq!(called.outcome, Ok(expected["finishResult"].clone()));
            assert_eq!(session.requests.get(), index + 1);
            assert!(called.updates.is_empty());
        }
    });
}

/// Every admitted environment reaches both progress updates and the final details.
#[test]
fn deployment_progress_keeps_environment_and_order() {
    local(async {
        let session = Session::new(None).await;
        let expected = corpus();
        for (index, row) in expected["deployments"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            let environment = row["environment"].clone();
            let signal = Cancellation::new();
            signal.abort();
            let called = run_tool(
                session.runner(),
                "deploy_and_exit",
                json!({"environment": environment}),
                Some(signal),
                None,
            )
            .await;
            assert_eq!(called.updates.len(), 2);
            assert_eq!(called.updates[0]["content"][0]["text"], row["progress"]);
            assert_eq!(called.updates[0]["details"], json!({}));
            assert_eq!(
                called.updates[1]["content"][0]["text"],
                expected["deploymentComplete"]
            );
            assert_eq!(called.updates[1]["details"], json!({}));
            assert_eq!(session.requests.get(), index + 1);
            assert_eq!(called.outcome, Ok(deployed(&environment)));
        }
    });
}

/// Without a progress handle the deployment still requests shutdown and answers.
#[test]
fn deployment_without_progress_keeps_result() {
    local(async {
        let session = Session::new(None).await;
        let expected = corpus();
        let result = call_tool(
            session.runner(),
            "deploy_and_exit",
            expected["withoutProgress"]["params"].clone(),
        )
        .await;
        assert_eq!(result, Ok(expected["withoutProgress"]["result"].clone()));
        assert_eq!(session.requests.get(), 1);
    });
}

/// A cancellation raised while progress runs keeps both updates and the result.
#[test]
fn deployment_survives_cancellation_during_progress() {
    local(async {
        let session = Session::new(None).await;
        let signal = Cancellation::new();
        let raised = signal.clone();
        let on_update: Rc<dyn Fn()> = Rc::new(move || raised.abort());
        let called = run_tool(
            session.runner(),
            "deploy_and_exit",
            json!({"environment": "staging"}),
            Some(signal.clone()),
            Some(on_update),
        )
        .await;
        assert!(
            signal.is_aborted(),
            "the application raised the signal during progress"
        );
        assert_eq!(called.updates.len(), 2, "both updates still arrive");
        assert_eq!(
            called.updates[1]["content"][0]["text"],
            corpus()["deploymentComplete"]
        );
        assert_eq!(session.requests.get(), 1);
        assert_eq!(called.outcome, Ok(deployed(&json!("staging"))));
    });
}

/// Argument edits reach the deployment's own member checks: a mutated member, an added
/// member, a replaced document and a deleted member.
#[test]
fn deployment_arguments_edits_keep_the_member_checks() {
    local(async {
        let session = Session::new(None).await;
        let expected = corpus();
        let mutated = expected["deployments"][0]["environment"].clone();
        let outcome = call_tool(
            session.runner(),
            "deploy_and_exit",
            json!({"environment": mutated}),
        )
        .await;
        assert_eq!(outcome.unwrap()["details"], json!({"environment": mutated}));
        let added = call_tool(
            session.runner(),
            "deploy_and_exit",
            expected["withoutProgress"]["params"].clone(),
        )
        .await;
        assert_eq!(added, Ok(expected["withoutProgress"]["result"].clone()));
        let requested = session.requests.get();
        for (edited, needle) in [(json!(5), "invalid type"), (json!({}), "missing field")] {
            let called = run_tool(session.runner(), "deploy_and_exit", edited, None, None).await;
            let error = called.outcome.unwrap_err();
            assert!(error.contains(needle), "{error}");
            assert!(called.updates.is_empty(), "{error}");
        }
        assert_eq!(
            session.requests.get(),
            requested,
            "no shutdown for rejected arguments"
        );
    });
}

/// The built example runs the command and both tools one after another.
#[test]
fn shutdown_component_runs_every_registration() {
    local(async {
        let session = Session::new(None).await;
        let expected = corpus();
        session.quit("ignored Ω").await.unwrap();
        let finished = call_tool(session.runner(), "finish_and_exit", json!({})).await;
        assert_eq!(finished, Ok(expected["finishResult"].clone()));
        let called = run_tool(
            session.runner(),
            "deploy_and_exit",
            json!({"environment": "staging"}),
            None,
            None,
        )
        .await;
        assert_eq!(called.updates.len(), 2);
        assert_eq!(
            called.updates[0]["content"][0]["text"],
            expected["deployments"][1]["progress"]
        );
        assert_eq!(called.outcome, Ok(deployed(&json!("staging"))));
        let bare = call_tool(
            session.runner(),
            "deploy_and_exit",
            expected["withoutProgress"]["params"].clone(),
        )
        .await;
        assert_eq!(bare, Ok(expected["withoutProgress"]["result"].clone()));
        assert_eq!(session.requests.get(), 4);
    });
}

/// A refused shutdown surfaces the application's message from the command and from both tools,
/// and the deployment still delivers its updates first.
#[test]
fn shutdown_example_stops_on_callback_errors() {
    local(async {
        let refusal = corpus()["refusals"]["shutdown"]
            .as_str()
            .unwrap()
            .to_owned();
        let session = Session::new(Some(&refusal)).await;
        assert_eq!(session.quit("").await, Err(refusal.clone()));
        let finished = run_tool(session.runner(), "finish_and_exit", json!({}), None, None).await;
        assert_eq!(finished.outcome, Err(refusal.clone()));
        assert!(finished.updates.is_empty());
        let deployed = run_tool(
            session.runner(),
            "deploy_and_exit",
            json!({"environment": "staging"}),
            None,
            None,
        )
        .await;
        assert_eq!(deployed.outcome, Err(refusal));
        assert_eq!(deployed.updates.len(), 2);
        assert_eq!(session.requests.get(), 0);
    });
}

/// A runtime that refuses registrations fails the component's load with the refusal.
#[test]
fn shutdown_example_stops_on_registration_error() {
    local(async {
        let message = corpus()["refusals"]["registration"]
            .as_str()
            .unwrap()
            .to_owned();
        let harness = super::support::harness();
        harness.runtime.invalidate(Some(message.clone()));
        let loaded = harness
            .load(vec![super::support::component(COMPONENT)])
            .await
            .unwrap();
        let errors: Vec<_> = loaded
            .errors
            .iter()
            .map(|error| error.error.clone())
            .collect();
        assert_eq!(errors, [format!("Failed to load extension: {message}")]);
        assert!(loaded.runner.get_registered_commands().is_empty());
        assert!(loaded.runner.get_all_registered_tools().is_empty());
    });
}
