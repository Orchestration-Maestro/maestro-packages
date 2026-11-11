//! The shipped session-name command loaded into the production engine.
use super::support::{Recorder, TERMINAL, bind, harness, load, local};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// The variable that names the built session-name component.
const COMPONENT: &str = "MAESTRO_SESSION_NAME_COMPONENT";

/// Recorded command metadata and invocation cases.
fn corpus() -> Value {
    serde_json::from_str(include_str!("session_name_expected.json")).unwrap()
}

/// The recorded cases `keep` selects.
fn selected(keep: impl Fn(&Value) -> bool) -> Vec<Value> {
    corpus()["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| keep(case))
        .cloned()
        .collect()
}

/// Whether a case writes a name.
fn writes_a_name(case: &Value) -> bool {
    case["expected"]["calls"][0]["operation"] == "set" && case["expected"]["error"].is_null()
}

/// Whether a case reads and shows the name.
fn shows_the_name(case: &Value) -> bool {
    case["expected"]["calls"][0]["operation"] == "get" && case["expected"]["error"].is_null()
}

/// The host operation a recorded call is, as the test's request log spells it.
fn request(call: &Value) -> Value {
    match call["operation"].as_str().unwrap() {
        "set" => json!(["set", call["value"]]),
        "get" => json!(["get", null]),
        _ => json!(["notify", [call["message"], "Info"]]),
    }
}

/// A loaded component whose session name the test controls.
struct Instance {
    /// Keeps the engine alive.
    _harness: maestro_extensions_wasmtime::test_support::Harness,
    /// The loaded runner.
    loaded: maestro_extensions_wasmtime::test_support::Loaded,
    /// The UI and operation log.
    ui: Rc<Recorder>,
    /// The session name the host reports.
    current: Rc<RefCell<Option<String>>>,
}

impl Instance {
    /// Loads the component and serves the session-name operations.
    async fn new() -> Self {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let ui = Recorder::install_on(&loaded.runner, TERMINAL);
        let current = Rc::new(RefCell::new(None));
        bind(&loaded.runner, |actions, _| {
            let (log, name) = (ui.clone(), current.clone());
            actions.set_session_name = Rc::new(move |value| {
                log.record("set", json!(value))?;
                *name.borrow_mut() = Some(value);
                Ok(())
            });
            let (log, name) = (ui.clone(), current.clone());
            actions.get_session_name = Rc::new(move || {
                log.record("get", Value::Null)?;
                Ok(name.borrow().clone())
            });
        });
        Self {
            _harness: harness,
            loaded,
            ui,
            current,
        }
    }

    /// Runs one recorded case and checks its calls, in order, and its failure.
    async fn check(&self, case: &Value) {
        let input = &case["input"];
        self.ui.disarm();
        *self.current.borrow_mut() = input["current"].as_str().map(str::to_owned);
        let refusal = case["expected"]["error"].as_str();
        if let (Some(operation), Some(error)) = (input["reject"].as_str(), refusal) {
            self.ui.reject(operation, error);
        }
        let command = self
            .loaded
            .runner
            .get_command("session-name")
            .unwrap()
            .command;
        let outcome = (command.handler)(
            input["args"].as_str().unwrap().into(),
            self.loaded.runner.create_command_context(),
        )
        .await;
        let name = &case["name"];
        let calls: Vec<Value> = case["expected"]["calls"]
            .as_array()
            .unwrap()
            .iter()
            .map(request)
            .collect();
        assert_eq!(self.ui.take(), calls, "{name}");
        assert_eq!(
            outcome.err().map(|error| error.to_string()).as_deref(),
            refusal,
            "{name}"
        );
    }
}

/// The registered command is the only registration, with the recorded metadata.
#[test]
fn session_name_registers_command_metadata() {
    local(async {
        let instance = Instance::new().await;
        let runner = &instance.loaded.runner;
        let recorded = &corpus()["metadata"];
        let commands = runner.get_registered_commands();
        assert_eq!(commands.len(), 1);
        let command = &commands[0].command;
        assert_eq!(json!(command.name), recorded["name"]);
        assert_eq!(json!(command.description), recorded["description"]);
        assert_eq!(
            command.get_argument_completions.is_some(),
            recorded["completions"]
        );
        assert!(runner.get_all_registered_tools().is_empty());
        assert!(
            instance.ui.take().is_empty(),
            "the factory reaches nothing else"
        );
    });
}

/// Every nonempty trimmed argument is written and then announced.
#[test]
fn session_name_sets_trimmed_names() {
    local(async {
        let instance = Instance::new().await;
        let cases = selected(writes_a_name);
        assert_eq!(cases.len(), 8, "the recorded write cases");
        for case in &cases {
            instance.check(case).await;
        }
    });
}

/// A blank argument reads the name now in effect and shows it unchanged.
#[test]
fn session_name_shows_current_or_absent_name() {
    local(async {
        let instance = Instance::new().await;
        let cases = selected(shows_the_name);
        assert_eq!(cases.len(), 10, "the recorded read cases");
        for case in &cases {
            instance.check(case).await;
        }
    });
}

/// Each refused host operation is returned unchanged and stops the command, and a refused
/// registration fails the load.
#[test]
fn session_name_propagates_host_errors() {
    local(async {
        let refusals = selected(|case| !case["expected"]["error"].is_null());
        assert_eq!(refusals.len(), 4, "the recorded refusal cases");
        let instance = Instance::new().await;
        for case in refusals
            .iter()
            .filter(|case| case["input"]["reject"] != "register")
        {
            instance.check(case).await;
        }
        let harness = harness();
        let error = refusals[0]["expected"]["error"].as_str().unwrap();
        harness.runtime.invalidate(Some(error.into()));
        let path = super::support::component(COMPONENT);
        let loaded = harness.load(vec![path.clone()]).await.unwrap();
        let errors: Vec<_> = loaded
            .errors
            .iter()
            .map(|failure| failure.error.clone())
            .collect();
        assert_eq!(errors, [format!("Failed to load extension: {error}")]);
        assert!(loaded.runner.get_registered_commands().is_empty());
    });
}
