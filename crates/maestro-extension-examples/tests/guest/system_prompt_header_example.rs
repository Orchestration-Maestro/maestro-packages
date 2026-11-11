//! The shipped system-prompt status component loaded into the production engine.
use super::support::{Bindings, Failures, Instance, Recorder, emit, local};
use maestro_extensions_wasmtime::test_support::maestro_extensions::{
    AgentStartEvent, SessionShutdownEvent,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// The variable that names the built status component.
const COMPONENT: &str = "MAESTRO_SYSTEM_PROMPT_HEADER_COMPONENT";

/// One recorded invocation of a registered handler.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Step {
    /// Event the application delivers.
    event: String,
    /// Prompt the application reports while the handler runs.
    prompt: String,
    /// Failure the prompt reader reports instead of a prompt.
    #[serde(rename = "getError")]
    get_error: Option<String>,
    /// Failure the next status request reports.
    #[serde(rename = "setError")]
    set_error: Option<String>,
    /// Operands of each status request the UI must record.
    requests: Vec<(String, Option<String>)>,
    /// Message the handler must fail with instead of completing.
    error: Option<String>,
}

/// One recorded case: the test that owns it and its invocations on one registration.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Case name the assertions report.
    name: String,
    /// Test group that owns the case.
    group: String,
    /// Recorded invocations in order.
    steps: Vec<Step>,
}

/// The recorded cases.
fn corpus() -> Vec<Case> {
    serde_json::from_str(include_str!("system_prompt_header_expected.json")).unwrap()
}

/// One loaded component, the prompt the application reports and what the UI records.
struct Session {
    /// The loaded component.
    instance: Instance,
    /// The recording UI.
    ui: Rc<Recorder>,
    /// The failures the handlers reported.
    failures: Failures,
    /// The prompt reader's current answer.
    prompt: Rc<RefCell<Result<String, String>>>,
}

impl Session {
    /// Loads a fresh component bound to a prompt the test changes.
    async fn new() -> Self {
        let instance = Instance::new(COMPONENT).await;
        let ui = Recorder::install(instance.runner());
        let failures = Failures::collect(instance.runner());
        let prompt = Rc::new(RefCell::new(Ok(String::new())));
        let mut bindings = Bindings::new();
        let reader = prompt.clone();
        bindings.context.get_system_prompt =
            Rc::new(move || reader.borrow().clone().map_err(Into::into));
        bindings.apply(instance.runner());
        Self {
            instance,
            ui,
            failures,
            prompt,
        }
    }

    /// Delivers one recorded step and asserts its ending and status requests.
    async fn step(&self, case: &str, step: &Step) {
        *self.prompt.borrow_mut() = match &step.get_error {
            Some(error) => Err(error.clone()),
            None => Ok(step.prompt.clone()),
        };
        self.ui.disarm();
        if let Some(error) = &step.set_error {
            self.ui.reject("setStatus", error);
        }
        let runner = self.instance.runner();
        match step.event.as_str() {
            "agent_start" => {
                emit::<AgentStartEvent>(runner, &json!({})).await;
            }
            _ => {
                emit::<SessionShutdownEvent>(runner, &json!({"reason": "quit"})).await;
            }
        }
        let context = format!("{case} {}", step.event);
        assert_eq!(self.failures.take().pop(), step.error, "{context}");
        let seen: Vec<Value> = self
            .ui
            .take()
            .into_iter()
            .map(|call| {
                assert_eq!(call[0], "setStatus", "{context}");
                call[1].clone()
            })
            .collect();
        assert_eq!(json!(seen), json!(step.requests), "{context}");
    }
}

/// Runs every recorded case of `group`, each on a fresh registration.
fn run_group(group: Option<&str>) {
    local(async {
        for case in corpus()
            .into_iter()
            .filter(|case| group.is_none_or(|group| case.group == group))
        {
            let session = Session::new().await;
            for step in &case.steps {
                session.step(&case.name, step).await;
            }
        }
    });
}

/// Loading registers the two lifecycle handlers and requests no status.
#[test]
fn system_prompt_status_registers_lifecycle_handlers() {
    local(async {
        let session = Session::new().await;
        let runner = session.instance.runner();
        assert!(runner.has_handlers("agent_start"));
        assert!(runner.has_handlers("session_shutdown"));
        assert!(session.ui.take().is_empty(), "loading requests no status");
    });
}

/// Every recorded prompt gives its exact UTF-16 length, including one changed after loading.
#[test]
fn system_prompt_status_reads_current_utf16_prompt() {
    run_group(Some("utf16"));
}

/// Shutdown clears the status key on each lifecycle path, including before any start.
#[test]
fn system_prompt_status_clears_on_shutdown() {
    run_group(Some("lifecycle"));
}

/// Application failures reach the caller unchanged and the next invocation still works.
#[test]
fn system_prompt_status_propagates_host_errors() {
    run_group(Some("errors"));
}

/// The built single-copy example produces every recorded answer.
#[test]
fn system_prompt_status_component_matches_lifecycle() {
    run_group(None);
}
