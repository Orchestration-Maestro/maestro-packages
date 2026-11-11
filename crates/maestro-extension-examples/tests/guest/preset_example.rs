//! The shipped preset component loaded into the production engine over a project directory.
//!
//! The project layer is the file under the session's directory. The global layer lives in the
//! user's own agent directory, which a test must not write, so the recorded global presets are
//! supplied as the first entries of the project file; a project entry replaces an earlier one in
//! place, exactly as it replaces a global one.
use super::support::{
    Bindings, Failures, Instance, Log, Recorder, before_agent_start, driven, emit, plain_rows,
    register_model, replace_branch,
};
use maestro_extensions_wasmtime::test_support::maestro_extensions::{
    SessionStartEvent, SessionTreeEvent, TurnStartEvent, types::FlagValue,
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// The variable that names the built preset component.
const COMPONENT: &str = "MAESTRO_PRESET_COMPONENT";

/// The recorded global presets.
const GLOBAL: &str = include_str!("preset_data/global.json");
/// The recorded project presets.
const PROJECT: &str = include_str!("preset_data/project.json");
/// A project file whose preset has an impossible thinking level.
const INVALID: &str = include_str!("preset_data/invalid.json");
/// A project file that is not JSON.
const MALFORMED: &str = include_str!("preset_data/malformed.json");

/// The names the layered presets list, in order.
const NAMES: &str = "zeta, plan, empty, invalidTools, missingModel, alpha";

/// The members of a JSON object's text, without its braces.
fn members(object: &str) -> &str {
    let object = object.trim();
    let object = object.strip_prefix('{').unwrap_or(object);
    object.strip_suffix('}').unwrap_or(object)
}

/// The global presets with the project presets layered over them.
fn layered() -> String {
    format!("{{{},{}}}", members(GLOBAL), members(PROJECT))
}

/// One persisted preset choice on the selected branch.
fn saved(name: &Value) -> Value {
    json!({"customType":"preset-state","data":{"name":name}})
}

/// Prompt input retains the assembled text exactly.
fn prompt() -> Value {
    json!({"type":"before_agent_start","prompt":"question","systemPrompt":" base \n","systemPromptOptions":{"cwd":"/work"}})
}

/// The recorded project `plan` instructions appended to the base prompt.
fn planned() -> Value {
    json!({"systemPrompt":" base \n\n\nProject instructions with more than thirty characters."})
}

/// A notification as the UI records it.
fn notice(message: &str, kind: &str) -> Value {
    json!(["notify", [message, kind]])
}

/// The next model selection waits here until the test opens the gate.
struct Hold {
    /// Told when the selection has been requested.
    entered: tokio::sync::oneshot::Sender<()>,
    /// Opened by the test.
    gate: tokio::sync::oneshot::Receiver<()>,
}

/// What the application was asked to change.
#[derive(Default)]
struct Changes {
    /// Models selected, as `provider/id`.
    models: Vec<String>,
    /// Thinking levels selected.
    thinking: Vec<Value>,
    /// Active tool lists selected.
    tools: Vec<Value>,
}

/// One loaded preset component over a project directory.
struct Session {
    /// The scratch project directory, kept until the session ends.
    _directory: maestro_test_directory::Scratch,
    /// The loaded component.
    instance: Instance,
    /// The recording UI.
    ui: Rc<Recorder>,
    /// The failures the callbacks reported.
    failures: Failures,
    /// The changes the application was asked for.
    changes: Rc<RefCell<Changes>>,
    /// The custom entries the component saved.
    entries: Log,
    /// Whether the application accepts a model selection.
    accepts_model: Rc<Cell<bool>>,
    /// The hold the next model selection waits on, when the test set one.
    hold: Rc<RefCell<Option<Hold>>>,
    /// The identity of the model the session started with, as `provider/id`.
    original: String,
}

/// The application operations the example reads and changes, each change recorded.
fn settings(
    changes: &Rc<RefCell<Changes>>,
    accepts: &Rc<Cell<bool>>,
    hold: &Rc<RefCell<Option<Hold>>>,
) -> Bindings {
    let mut bindings = Bindings::new();
    let (recorded, accepts, hold) = (changes.clone(), accepts.clone(), hold.clone());
    bindings.actions.set_model = Rc::new(move |model| {
        {
            let model = model.read().unwrap();
            let name = format!("{}/{}", model.provider, model.id);
            recorded.borrow_mut().models.push(name);
        }
        let (pending, accepted) = (hold.borrow_mut().take(), accepts.get());
        Box::pin(async move {
            if let Some(pending) = pending {
                let _ = pending.entered.send(());
                let _ = pending.gate.await;
            }
            Ok(accepted)
        })
    });
    let recorded = changes.clone();
    bindings.actions.set_thinking_level = Rc::new(move |level| {
        recorded.borrow_mut().thinking.push(json!(level));
        Ok(())
    });
    bindings.actions.get_thinking_level =
        Rc::new(|| Ok(serde_json::from_value(json!("medium")).unwrap()));
    let recorded = changes.clone();
    bindings.actions.set_active_tools = Rc::new(move |tools| {
        recorded.borrow_mut().tools.push(json!(tools));
        Ok(())
    });
    bindings.actions.get_active_tools = Rc::new(|| Ok(vec!["bash".into()]));
    bindings.actions.get_all_tools = Rc::new(|| {
        ["read", "extra"]
            .into_iter()
            .map(|name| {
                serde_json::from_value(json!({"name":name,"description":name,"parameters":{},
                    "sourceInfo":{"path":"/tools","source":"local","scope":"project","origin":"top-level"}}))
                    .map_err(Into::into)
            })
            .collect()
    });
    bindings
}

impl Session {
    /// Loads the component over a project that holds `presets`, or none when absent.
    async fn new(presets: Option<&str>, interactive: bool) -> Self {
        let directory = maestro_test_directory::Scratch::new().unwrap();
        if let Some(presets) = presets {
            std::fs::create_dir_all(directory.path().join(".maestro")).unwrap();
            std::fs::write(directory.path().join(".maestro/presets.json"), presets).unwrap();
        }
        let instance = Instance::in_directory(COMPONENT, directory.path().to_str().unwrap()).await;
        register_model(&instance.harness, "configured", "custom-model");
        let ui = if interactive {
            Recorder::install_on(instance.runner(), (100, 30))
        } else {
            Recorder::install(instance.runner())
        };
        let failures = Failures::collect(instance.runner());
        let changes = Rc::new(RefCell::new(Changes::default()));
        let accepts_model = Rc::new(Cell::new(true));
        let hold = Rc::new(RefCell::new(None));
        let first = &instance.harness.faux.models[0];
        let original_model = instance
            .harness
            .model_registry
            .find(&first.provider, &first.id)
            .unwrap();
        let original = format!("{}/{}", first.provider, first.id);
        let mut bindings = settings(&changes, &accepts_model, &hold);
        bindings.context.get_model = Rc::new(move || Ok(Some(original_model.clone())));
        let entries = bindings.record_entries(instance.harness.session_manager.clone());
        bindings.apply(instance.runner());
        Self {
            _directory: directory,
            instance,
            ui,
            failures,
            changes,
            entries,
            accepts_model,
            hold,
            original,
        }
    }

    /// Replaces the session's branch with the saved names and delivers `session_start`.
    async fn start(&self, branch: &[Value]) {
        replace_branch(&self.instance.harness.session_manager, branch);
        emit::<SessionStartEvent>(self.instance.runner(), &json!({"reason": "startup"})).await;
    }

    /// Runs the `preset` command with `args`.
    async fn command(&self, args: &str) -> Result<(), String> {
        let command = self
            .instance
            .runner()
            .get_command("preset")
            .unwrap()
            .command;
        (command.handler)(args.into(), self.instance.runner().create_command_context())
            .await
            .map_err(|error| error.to_string())
    }

    /// Runs the cycle shortcut.
    async fn cycle(&self) {
        let shortcuts = self.instance.runner().get_shortcuts(&Vec::new());
        let shortcut = shortcuts.get("ctrl+shift+u").expect("the cycle shortcut");
        let handler = shortcut.read().unwrap().handler.clone();
        handler(self.instance.runner().create_context())
            .await
            .unwrap();
    }

    /// Delivers a `turn_start`.
    async fn turn(&self) {
        let event = json!({"turnIndex": 0.0, "timestamp": 0.0});
        emit::<TurnStartEvent>(self.instance.runner(), &event).await;
    }

    /// The system prompt the handlers answer for the recorded prompt.
    async fn answer(&self) -> Value {
        before_agent_start(self.instance.runner(), &prompt()).await
    }

    /// The notifications recorded so far.
    fn notices(&self) -> Vec<Value> {
        self.ui
            .take()
            .into_iter()
            .filter(|call| call[0] == "notify")
            .collect()
    }

    /// The saved choices, oldest first.
    fn saves(&self) -> Vec<Value> {
        self.entries.borrow().clone()
    }

    /// Whether no model, thinking level or tool list has been selected.
    fn settings_untouched(&self) -> bool {
        let changes = self.changes.borrow();
        changes.models.is_empty() && changes.thinking.is_empty() && changes.tools.is_empty()
    }

    /// The models selected.
    fn models(&self) -> Vec<String> {
        self.changes.borrow().models.clone()
    }
}

/// The text of the status the UI was last asked to show, without control sequences.
fn status_text(calls: &[Value]) -> Option<String> {
    calls
        .iter()
        .rev()
        .find(|call| call[0] == "setStatus")
        .and_then(|call| call[1][1].as_str())
        .map(|text| plain_rows(&[text.to_owned()]).remove(0))
}

/// The branch restores the newest saved name and its instructions, without applying settings.
#[test]
fn preset_layers_restore_branch_instructions_without_reapplying_settings() {
    driven(async {
        let session = Session::new(Some(&layered()), false).await;
        session
            .start(&[saved(&json!("zeta")), saved(&json!("plan"))])
            .await;
        assert_eq!(session.answer().await, planned());
        assert!(session.settings_untouched());
        assert_eq!(
            status_text(&session.ui.take()).as_deref(),
            Some("preset:plan")
        );
        session.start(&[]).await;
        assert_eq!(
            session.answer().await,
            Value::Null,
            "a new branch must not retain the previous preset"
        );
        assert!(session.failures.take().is_empty());
    });
}

/// Commands apply configured models, valid tools and shared whitespace rules.
#[test]
fn preset_applies_configured_models_valid_tools_and_shared_whitespace() {
    driven(async {
        let session = Session::new(Some(&layered()), false).await;
        session.start(&[]).await;
        let commands = session.instance.runner().get_registered_commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].command.name, "preset");
        assert_eq!(
            json!(commands[0].command.description),
            "Switch preset configuration"
        );
        assert!(commands[0].command.get_argument_completions.is_none());
        let flags = session.instance.runner().get_flags();
        assert_eq!(flags.keys().collect::<Vec<_>>(), ["preset"]);
        session.turn().await;
        assert!(session.saves().is_empty(), "no preset, no saved choice");
        session.ui.take();
        session.command("\u{feff} plan \u{feff}").await.unwrap();
        assert_eq!(session.models(), ["configured/custom-model"]);
        assert_eq!(session.changes.borrow().thinking, [json!("high")]);
        assert_eq!(session.changes.borrow().tools, [json!(["read", "extra"])]);
        assert_eq!(
            session.notices(),
            [
                notice("Preset \"plan\": Unknown tools: missing", "Warning"),
                notice("Preset \"plan\" activated", "Info"),
            ]
        );
        session.command("\u{85}plan\u{85}").await.unwrap();
        assert_eq!(
            session.notices(),
            [notice(
                &format!("Unknown preset \"\u{85}plan\u{85}\". Available: {NAMES}"),
                "Error"
            )]
        );
    });
}

/// A missing model, a refused key and unusable tool lists warn and keep the other settings.
#[test]
fn preset_warns_about_missing_models_keys_and_unusable_tool_lists() {
    driven(async {
        let session = Session::new(Some(&layered()), false).await;
        session.start(&[]).await;
        session.ui.take();
        session.command("missingModel").await.unwrap();
        assert!(session.notices().contains(&notice(
            "Preset \"missingModel\": Model configured/absent not found",
            "Warning"
        )));
        session.accepts_model.set(false);
        session.command("plan").await.unwrap();
        assert!(session.notices().contains(&notice(
            "Preset \"plan\": No API key for configured/custom-model",
            "Warning"
        )));
        session.accepts_model.set(true);
        let tools = session.changes.borrow().tools.clone();
        for name in ["invalidTools", "alpha", "empty"] {
            session.command(name).await.unwrap();
        }
        assert_eq!(
            session.changes.borrow().tools,
            tools,
            "empty or wholly invalid lists do not disable tools"
        );
        assert_eq!(
            session.changes.borrow().thinking.last(),
            Some(&json!("off"))
        );
        assert!(session.failures.take().is_empty());
    });
}

/// Cycling restores the original settings and persists the cleared branch state.
#[test]
fn preset_cycles_restore_original_settings_and_persist_cleared_branch_state() {
    driven(async {
        let session = Session::new(Some(&layered()), false).await;
        session.start(&[]).await;
        session.command("plan").await.unwrap();
        session.turn().await;
        session.cycle().await; // plan -> zeta, in sorted order rather than file order.
        session.cycle().await; // zeta -> none, restoring the first snapshot.
        assert_eq!(session.models().last(), Some(&session.original));
        assert_eq!(
            session.changes.borrow().thinking.last(),
            Some(&json!("medium"))
        );
        assert_eq!(
            session.changes.borrow().tools.last(),
            Some(&json!(["bash"]))
        );
        assert_eq!(
            session.notices().last(),
            Some(&notice("Preset cleared, defaults restored", "Info"))
        );
        session.turn().await;
        assert_eq!(
            session.saves(),
            [
                json!({"type":"preset-state","data":{"name":"plan"}}),
                json!({"type":"preset-state","data":{"name":null}}),
            ]
        );
        replace_branch(
            &session.instance.harness.session_manager,
            &[saved(&json!("plan")), saved(&Value::Null)],
        );
        emit::<SessionTreeEvent>(
            session.instance.runner(),
            &json!({"newLeafId": "next", "oldLeafId": "old"}),
        )
        .await;
        assert_eq!(session.answer().await, Value::Null);
        session.cycle().await; // none -> alpha.
        assert_eq!(
            session.notices().last(),
            Some(&notice("Preset \"alpha\" activated", "Info"))
        );
    });
}

/// An invalid, malformed or absent project file defines no presets.
#[test]
fn preset_rejects_invalid_layers_without_losing_the_other_file() {
    driven(async {
        let warning = notice(
            "No presets defined. Add presets to ~/.maestro/agent/presets.json or .maestro/presets.json",
            "Warning",
        );
        for file in [Some(INVALID), Some(MALFORMED), None] {
            let session = Session::new(file, false).await;
            session.start(&[]).await;
            session.ui.take();
            session.command(" ").await.unwrap();
            session.cycle().await;
            assert_eq!(session.notices(), [warning.clone(), warning.clone()]);
            session.command("unknown").await.unwrap();
            assert_eq!(
                session.notices(),
                [notice(
                    "Unknown preset \"unknown\". Available: (none defined)",
                    "Error"
                )]
            );
        }
        let session = Session::new(Some(PROJECT), false).await;
        session.start(&[]).await;
        session.command("alpha").await.unwrap();
        assert_eq!(
            session.answer().await,
            json!({"systemPrompt":" base \n\n\nAlpha"})
        );
    });
}

/// The startup flag is applied before the session start returns and overrides the saved name.
#[test]
fn preset_startup_flag_is_applied_before_returning_and_overrides_saved_name() {
    driven(async {
        let session = Session::new(Some(&layered()), false).await;
        session
            .instance
            .runner()
            .set_flag_value("preset".into(), FlagValue::String("plan".into()));
        session.start(&[saved(&json!("zeta"))]).await;
        assert_eq!(session.models(), ["configured/custom-model"]);
        assert_eq!(session.changes.borrow().thinking, [json!("high")]);
        assert_eq!(session.changes.borrow().tools, [json!(["read", "extra"])]);
        assert_eq!(session.answer().await, planned());
        assert!(
            session
                .notices()
                .contains(&notice("Preset \"plan\" activated", "Info"))
        );
        session
            .instance
            .runner()
            .set_flag_value("preset".into(), FlagValue::String("unknown".into()));
        session.start(&[saved(&json!("zeta"))]).await;
        assert_eq!(session.answer().await, Value::Null);
        assert_eq!(
            session.notices().last(),
            Some(&notice(
                &format!("Unknown preset \"unknown\". Available: {NAMES}"),
                "Warning"
            ))
        );
    });
}

/// The selector lists each preset with its description, marks the active one, and restores the
/// fallback tools when cleared.
#[test]
fn preset_selector_renders_descriptions_marks_active_and_restores_fallback_tools() {
    driven(async {
        let session = Session::new(Some(&layered()), true).await;
        session.start(&[saved(&json!("plan"))]).await;
        session.ui.take();
        let (closed, opened) = (session.ui.clone(), session.ui.opened());
        let runner = session.instance.runner().clone();
        let open = || {
            let runner = runner.clone();
            tokio::task::spawn_local(async move {
                let command = runner.get_command("preset").unwrap().command;
                (command.handler)(String::new(), runner.create_command_context())
                    .await
                    .map_err(|error| error.to_string())
            })
        };
        let view = open();
        closed.shown(opened + 1).await;
        let text = closed.rows(opened, 200).join("\n");
        assert!(text.contains("plan (active)"), "{text}");
        assert!(text.contains("configured/custom-model | thinking:high | tools:read,missing,extra | \"Project instructions with m..."), "{text}");
        assert!(text.contains("thinking:off | tools: | \"Alpha\""), "{text}");
        assert!(
            text.contains("Clear active preset, restore defaults"),
            "{text}"
        );
        closed.press(opened, "\x1b");
        view.await.unwrap().unwrap();
        assert!(
            session.notices().is_empty(),
            "cancel leaves the current preset alone"
        );
        let view = open();
        closed.shown(opened + 2).await;
        closed.press(opened + 1, "\x1b[A");
        closed.press(opened + 1, "\r");
        view.await.unwrap().unwrap();
        assert_eq!(
            session.changes.borrow().tools,
            [json!(["read", "bash", "edit", "write"])]
        );
        assert_eq!(
            session.notices().last(),
            Some(&notice("Preset cleared, defaults restored", "Info"))
        );
        let view = open();
        closed.shown(opened + 3).await;
        closed.press(opened + 2, "\r");
        view.await.unwrap().unwrap();
        assert_eq!(
            session.notices().last(),
            Some(&notice("Preset \"zeta\" activated", "Info"))
        );
    });
}

/// A startup preset waits for its pending model selection before the first turn.
#[test]
fn preset_startup_waits_for_pending_model_before_the_first_turn() {
    driven(async {
        let session = Session::new(Some(&layered()), false).await;
        let (entered, entered_signal) = tokio::sync::oneshot::channel();
        let (open, gate) = tokio::sync::oneshot::channel();
        session.hold.replace(Some(Hold { entered, gate }));
        session
            .instance
            .runner()
            .set_flag_value("preset".into(), FlagValue::String("plan".into()));
        let runner = session.instance.runner().clone();
        let start = tokio::task::spawn_local(async move {
            emit::<SessionStartEvent>(&runner, &json!({"reason": "startup"})).await;
        });
        entered_signal.await.unwrap();
        assert!(
            !start.is_finished(),
            "startup cannot complete while model selection is pending"
        );
        assert_eq!(session.models(), ["configured/custom-model"]);
        assert!(session.changes.borrow().thinking.is_empty());
        assert!(session.changes.borrow().tools.is_empty());
        open.send(()).unwrap();
        start.await.unwrap();
        assert_eq!(session.models(), ["configured/custom-model"]);
        assert_eq!(session.changes.borrow().thinking, [json!("high")]);
        assert_eq!(session.changes.borrow().tools, [json!(["read", "extra"])]);
        assert_eq!(session.answer().await, planned());
        session.turn().await;
        assert_eq!(
            session.saves(),
            [json!({"type":"preset-state","data":{"name":"plan"}})]
        );
    });
}
