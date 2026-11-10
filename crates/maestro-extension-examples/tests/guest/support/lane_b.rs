//! Recording pieces the lane b guest tests share: a UI that records each request, the failures
//! a runner reports, typed event delivery from JSON and a runtime with a paused clock.
use super::support::{harness, load};
use maestro_extensions_wasmtime::test_support::maestro_extensions::{
    ComponentHandle, ToolRenderContext, ToolRenderEnvironment, ToolRenderPhase, ToolRendererState,
    types::{
        CustomClose, CustomComponentFactory, CustomFactoryOutcome, CustomUIOptions,
        ExtensionComponent, ExtensionKeybindings,
    },
};
use maestro_extensions_wasmtime::test_support::maestro_extensions::{
    EventError, ExtensionContextActions, ExtensionExecution, ExtensionRunner, ExtensionUIContext,
    LiveTheme, Local, Presence, ProviderConfig, ProviderModelConfig, SessionManager, SharedHandle,
    execution::RunnerEmitEvent,
    types::{
        EditorFactory, EditorFactoryOutcome, ExtensionActions, ExtensionUIContextPort,
        ExtensionUIDialogOptions, ExtensionUIFuture, ExtensionWidgetOptions, FooterFactory,
        NotificationType, ProviderActions, WidgetContent, WorkingIndicatorOptions,
    },
};
use maestro_extensions_wasmtime::test_support::{
    Harness, HarnessOptions, Loaded, unconfigured_context_actions,
};
use maestro_tui::{
    KeybindingsManager, TUI, TUI_KEYBINDINGS, TerminalImage,
    test_support::{manual_runtime::ManualRuntime, recording_terminal::RecordingTerminal},
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};

/// Runs a test body on a current-thread runtime whose clock is paused, inside the local task
/// set that loaded components need. The clock moves only while every task waits, so a native
/// wait of any length ends at once.
pub fn timed<T>(test: impl Future<Output = T>) -> T {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, test)
}

/// Runs a test body like [`timed`], but the clock moves only when the test advances it.
///
/// A task that always has work keeps the runtime from parking, and a paused clock moves
/// forward on its own only while the runtime is parked.
pub fn stepped<T>(test: impl Future<Output = T>) -> T {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let busy = tokio::task::spawn_local(async {
            loop {
                tokio::task::yield_now().await;
            }
        });
        let outcome = test.await;
        busy.abort();
        outcome
    })
}

/// A harness with the component `variable` names already loaded.
pub struct Instance {
    /// The engine the component lives in; dropping it unregisters the scripted provider.
    pub harness: Harness,
    /// The runner over the loaded component.
    pub loaded: Loaded,
}

impl Instance {
    /// Loads the component `variable` names into a fresh engine.
    pub async fn new(variable: &str) -> Self {
        Self::with(variable, harness()).await
    }

    /// Loads the component `variable` names into a fresh engine whose session is in `cwd`.
    pub async fn in_directory(variable: &str, cwd: &str) -> Self {
        let options = HarnessOptions {
            agent_dir: maestro_test_process::run_agent_directory().unwrap_or_default(),
            ..HarnessOptions::default()
        };
        Self::with(variable, Harness::new(cwd, options).unwrap()).await
    }

    /// Loads the component `variable` names into `harness`.
    async fn with(variable: &str, harness: Harness) -> Self {
        let loaded = load(&harness, variable).await;
        Self { harness, loaded }
    }

    /// The runner over the loaded component.
    pub fn runner(&self) -> &Rc<ExtensionRunner<Local>> {
        &self.loaded.runner
    }
}

/// Runs a test body on a current-thread runtime with its drivers enabled and a real clock,
/// inside the local task set that loaded components need. Components that start processes need
/// the drivers.
pub fn driven<T>(test: impl Future<Output = T>) -> T {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, test)
}

/// Every request an extension made of the application's UI, in order, and the scripted answers
/// and refusals the next requests meet.
pub struct Recorder {
    /// Each request as `[method, operands]`, with the method in the host's camel case.
    calls: RefCell<Vec<Value>>,
    /// The method whose next request fails, and the failure.
    rejection: RefCell<Option<(String, String)>>,
    /// Answers the dialogs give, oldest first; a dialog with none is dismissed.
    answers: RefCell<VecDeque<Value>>,
    /// The theme the UI publishes.
    theme: LiveTheme,
    /// The terminal that custom components are built for, when the UI is interactive.
    stage: Option<Stage>,
    /// The custom components shown so far, oldest first.
    shown: RefCell<Vec<Rc<ExtensionComponent>>>,
    /// The editor the extension's factory last built, when it set one.
    editor: RefCell<Option<EditorFactoryOutcome>>,
}

/// A started terminal that custom components are built for.
pub struct Stage {
    /// The frame writer factories receive.
    tui: TUI,
    /// Keeps the terminal alive and lets the test send input.
    terminal: RecordingTerminal,
    /// Runs the work the frame writer scheduled.
    runtime: ManualRuntime,
}

impl Stage {
    /// A started terminal of `columns` by `rows`.
    fn new(columns: usize, rows: usize) -> Self {
        let terminal = RecordingTerminal::new(columns, rows);
        let runtime = ManualRuntime::new();
        let tui = TUI::new(
            terminal.handle(),
            runtime.handle(),
            TerminalImage::default(),
            Some(false),
        );
        tui.start().unwrap();
        runtime.settle().unwrap();
        Self {
            tui,
            terminal,
            runtime,
        }
    }
}

/// The application's shipped dark theme, published live.
pub fn dark() -> LiveTheme {
    LiveTheme::from_theme(maestro_theme::test_support::palette::palette("dark").unwrap())
}

/// Text without terminal control sequences, each row without trailing spaces.
pub fn plain(rows: &[String]) -> Vec<String> {
    rows.iter()
        .map(|row| maestro_app::strip_ansi(row).unwrap().trim_end().to_owned())
        .collect()
}

impl Recorder {
    /// Installs a recording UI on `runner` and returns it.
    pub fn install(runner: &ExtensionRunner<Local>) -> Rc<Self> {
        Self::install_with(runner, None)
    }

    /// Installs a recording UI that builds custom components for a terminal of `columns` by
    /// `rows` under the shipped dark theme.
    pub fn install_interactive(
        runner: &ExtensionRunner<Local>,
        columns: usize,
        rows: usize,
    ) -> Rc<Self> {
        Self::install_with(runner, Some(Stage::new(columns, rows)))
    }

    /// Installs the UI over an optional terminal.
    fn install_with(runner: &ExtensionRunner<Local>, stage: Option<Stage>) -> Rc<Self> {
        let theme = dark();
        let ui = Rc::new(Self {
            calls: RefCell::default(),
            rejection: RefCell::default(),
            answers: RefCell::default(),
            theme,
            stage,
            shown: RefCell::default(),
            editor: RefCell::default(),
        });
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
        ui
    }

    /// Waits until `count` custom components have been shown.
    pub async fn shown(&self, count: usize) {
        while self.shown.borrow().len() < count {
            tokio::task::yield_now().await;
        }
    }

    /// The rows the installed editor renders `width` cells wide, without control sequences.
    pub fn editor_rows(&self, width: usize) -> Vec<String> {
        let built = self.editor.borrow().clone().expect("an installed editor");
        plain(&built.editor().try_render(width).unwrap())
    }

    /// The terminal's recorded output so far.
    pub fn writes(&self) -> Vec<String> {
        self.stage
            .as_ref()
            .map_or_else(Vec::new, |stage| stage.terminal.writes())
    }

    /// How many custom components have been shown.
    pub fn opened(&self) -> usize {
        self.shown.borrow().len()
    }

    /// The `index`th custom component shown.
    pub fn component(&self, index: usize) -> ComponentHandle {
        self.shown.borrow()[index].component.clone()
    }

    /// The `index`th custom component's rows at `width`, without control sequences.
    pub fn rows(&self, index: usize, width: usize) -> Vec<String> {
        plain(&self.component(index).try_render(width).unwrap())
    }

    /// Delivers terminal input to the `index`th custom component.
    pub fn press(&self, index: usize, data: &str) {
        self.component(index)
            .input_handler()
            .expect("the component takes input")
            .handle_input(data);
    }

    /// Delivers terminal input to the `index`th custom component and returns the failure its
    /// handler reported.
    pub fn try_press(&self, index: usize, data: &str) -> Result<(), String> {
        self.component(index)
            .input_handler()
            .expect("the component takes input")
            .try_handle_input(data)
            .map_err(|error| error.to_string())
    }

    /// Runs the work the frame writer scheduled.
    pub fn settle(&self) {
        if let Some(stage) = &self.stage {
            stage.runtime.settle().unwrap();
        }
    }

    /// Makes the next request of `method` fail with `message`.
    pub fn reject(&self, method: &str, message: &str) {
        *self.rejection.borrow_mut() = Some((method.to_owned(), message.to_owned()));
    }

    /// Drops a refusal no request met.
    pub fn disarm(&self) {
        self.rejection.borrow_mut().take();
    }

    /// Queues the answer the next dialog gives.
    pub fn answer(&self, answer: Value) {
        self.answers.borrow_mut().push_back(answer);
    }

    /// Takes the requests recorded since the last call.
    pub fn take(&self) -> Vec<Value> {
        std::mem::take(&mut self.calls.borrow_mut())
    }

    /// Records a request and applies a pending refusal of its method.
    fn record(&self, method: &str, operands: Value) -> Result<(), EventError> {
        self.calls
            .borrow_mut()
            .push(Value::Array(vec![Value::from(method), operands]));
        let mut rejection = self.rejection.borrow_mut();
        match rejection.take_if(|(name, _)| name == method) {
            Some((_, message)) => Err(message.into()),
            None => Ok(()),
        }
    }

    /// Records a dialog and settles it with the next queued answer.
    fn dialog(&self, method: &str, operands: Value) -> ExtensionUIFuture<Option<Value>> {
        let outcome = self
            .record(method, operands)
            .map(|()| self.answers.borrow_mut().pop_front());
        Box::pin(std::future::ready(outcome))
    }
}

/// A notification severity in the host's own spelling.
fn kind(kind: Option<NotificationType>) -> Value {
    json!(kind.map(|kind| format!("{kind:?}")))
}

impl ExtensionUIContextPort for Recorder {
    fn theme(&self) -> LiveTheme {
        self.theme.clone()
    }

    fn select(
        &self,
        title: String,
        options: Vec<String>,
        _: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<Option<String>> {
        let settled = self.dialog("select", json!([title, options, null, null]));
        Box::pin(async move {
            Ok(settled
                .await?
                .and_then(|answer| answer.as_str().map(str::to_owned)))
        })
    }

    fn confirm(
        &self,
        title: String,
        message: String,
        _: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<bool> {
        let settled = self.dialog("confirm", json!([title, message, null, null]));
        Box::pin(async move { Ok(settled.await?.is_some_and(|answer| answer == json!(true))) })
    }

    fn notify(
        &self,
        message: String,
        notification: Option<NotificationType>,
    ) -> Result<(), EventError> {
        self.record("notify", json!([message, kind(notification)]))
    }

    fn set_status(&self, key: String, text: Option<String>) -> Result<(), EventError> {
        self.record("setStatus", json!([key, text]))
    }

    fn set_working_message(&self, message: Option<String>) -> Result<(), EventError> {
        self.record("setWorkingMessage", json!([message]))
    }

    fn set_working_indicator(
        &self,
        options: Option<WorkingIndicatorOptions>,
    ) -> Result<(), EventError> {
        let operands = options
            .map(|options| json!({"frames": options.frames, "intervalMs": options.interval_ms}));
        self.record("setWorkingIndicator", json!([operands]))
    }

    fn set_hidden_thinking_label(&self, label: Option<String>) -> Result<(), EventError> {
        self.record("setHiddenThinkingLabel", json!([label]))
    }

    fn set_widget(
        &self,
        key: String,
        content: Option<WidgetContent>,
        _: Option<ExtensionWidgetOptions>,
    ) -> Result<(), EventError> {
        let lines = content.map(|content| match content {
            WidgetContent::Lines(lines) => json!(lines),
            WidgetContent::Factory(_) => json!("factory"),
        });
        self.record("setWidget", json!([key, lines]))
    }

    fn set_title(&self, title: String) -> Result<(), EventError> {
        self.record("setTitle", json!([title]))
    }

    fn set_working_visible(&self, visible: bool) -> Result<(), EventError> {
        self.record("setWorkingVisible", json!([visible]))
    }

    fn set_footer(&self, factory: Option<FooterFactory>) -> Result<(), EventError> {
        self.record("setFooter", json!([factory.is_some()]))
    }

    fn set_editor_component(&self, factory: Option<EditorFactory>) -> Result<(), EventError> {
        let Some(stage) = &self.stage else {
            return Ok(());
        };
        let keys = ExtensionKeybindings {
            manager: KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Vec::new()),
            reload: None,
        };
        let theme = maestro_theme::get_editor_theme(self.theme.clone());
        let built = factory
            .map(|factory| factory.call((stage.tui.clone(), theme, keys)))
            .transpose()?;
        self.editor.replace(built);
        Ok(())
    }

    fn custom_component(
        &self,
        factory: CustomComponentFactory,
        _: Option<CustomUIOptions>,
    ) -> ExtensionUIFuture<()> {
        let Some(stage) = &self.stage else {
            return Box::pin(std::future::ready(Ok(())));
        };
        if let Err(error) = self.record("custom", json!([])) {
            return Box::pin(std::future::ready(Err(error)));
        }
        let (settle, settled) = tokio::sync::oneshot::channel();
        let settle = RefCell::new(Some(settle));
        let built: Rc<RefCell<Option<Rc<ExtensionComponent>>>> = Rc::default();
        let closing = built.clone();
        let close: CustomClose = Rc::new(move || {
            let Some(settle) = settle.borrow_mut().take() else {
                return Ok(false);
            };
            let _ = settle.send(());
            let component = closing.borrow_mut().take();
            if let Some(dispose) = component.and_then(|component| component.dispose.clone()) {
                dispose.call(())?;
            }
            Ok(true)
        });
        let keys = ExtensionKeybindings {
            manager: KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Vec::new()),
            reload: None,
        };
        let outcome = factory.call((stage.tui.clone(), self.theme.clone(), keys, close));
        match outcome {
            Ok(CustomFactoryOutcome::Ready(component)) => {
                let component = Rc::new(component);
                built.replace(Some(component.clone()));
                self.shown.borrow_mut().push(component);
                Box::pin(async move {
                    settled
                        .await
                        .map_err(|error| EventError::from(error.to_string()))
                })
            }
            Ok(CustomFactoryOutcome::Async(_)) => Box::pin(std::future::ready(Err(
                "an asynchronous view is not shown".into(),
            ))),
            Err(error) => Box::pin(std::future::ready(Err(error))),
        }
    }
}

/// The messages of the failures a runner's callbacks reported, oldest first.
#[derive(Clone, Default)]
pub struct Failures(Rc<RefCell<Vec<String>>>);

impl Failures {
    /// Starts collecting the failures `runner` reports.
    pub fn collect(runner: &ExtensionRunner<Local>) -> Self {
        let failures = Self::default();
        let sink = failures.0.clone();
        let remove = runner.on_error(Rc::new(move |error| {
            sink.borrow_mut().push(error.error.clone());
            Ok(())
        }));
        drop(remove);
        failures
    }

    /// Takes the messages collected since the last call.
    pub fn take(&self) -> Vec<String> {
        std::mem::take(&mut self.0.borrow_mut())
    }
}

/// Delivers an event written as JSON to the runner's handlers and returns their answer.
///
/// The `type` member a recorded event carries only names the event and is not part of its body.
pub async fn emit<V: RunnerEmitEvent + DeserializeOwned>(
    runner: &ExtensionRunner<Local>,
    event: &Value,
) -> Option<V::Result> {
    let mut body = event.clone();
    if let Some(object) = body.as_object_mut() {
        object.shift_remove("type");
    }
    let mut event: V = serde_json::from_value(body).unwrap();
    runner.emit(&mut event).await.unwrap()
}

/// The shared list a recording operation appends to.
pub type Log = Rc<RefCell<Vec<Value>>>;

/// The application operations a runner is bound to, each failing until a test replaces it.
pub struct Bindings {
    /// The session operations behind the extension API.
    pub actions: ExtensionActions<Local>,
    /// The operations behind every extension context.
    pub context: ExtensionContextActions,
}

impl Bindings {
    /// Operations that fail with the harness's unconfigured-capability errors.
    pub fn new() -> Self {
        Self {
            actions: Local::unbound_actions(),
            context: unconfigured_context_actions(),
        }
    }

    /// Records each custom message as `{"message": .., "options": ..}`.
    pub fn record_messages(&mut self) -> Log {
        let log = Log::default();
        let sink = log.clone();
        self.actions.send_message = Rc::new(move |message, options| {
            sink.borrow_mut()
                .push(json!({"message": message, "options": options}));
            Ok(())
        });
        log
    }

    /// Writes each custom entry to `session`, as an application does, and records it as
    /// `{"type": .., "data": ..}`.
    pub fn record_entries(&mut self, session: Rc<RefCell<SessionManager>>) -> Log {
        let log = Log::default();
        let sink = log.clone();
        self.actions.append_entry = Rc::new(move |custom_type, data| {
            sink.borrow_mut()
                .push(json!({"type": custom_type, "data": data}));
            let data = data.map_or(Presence::Missing, Presence::Present);
            session
                .borrow_mut()
                .append_custom_entry(&custom_type, data)
                .map(drop)
                .map_err(Into::into)
        });
        log
    }

    /// Binds the operations to `runner`, replacing the ones bound before.
    pub fn apply(self, runner: &ExtensionRunner<Local>) {
        runner
            .bind_core(self.actions, self.context, ProviderActions::default())
            .unwrap();
    }
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
    signal: Option<maestro_extensions_wasmtime::test_support::maestro_extensions::Cancellation>,
    on_update: Option<Rc<dyn Fn()>>,
) -> Called {
    let tools = runner.get_all_registered_tools();
    let registered = tools
        .iter()
        .find(|tool| tool.metadata().definition.name == name)
        .unwrap_or_else(|| panic!("no tool named {name}"));
    let tool = maestro_extensions_wasmtime::test_support::maestro_extensions::wrap_registered_tool(
        registered,
        runner.clone(),
    );
    let updates = Rc::new(RefCell::new(Vec::new()));
    let sink = updates.clone();
    let update: maestro_extensions_wasmtime::test_support::maestro_extensions::AgentToolUpdateCallback<Value, Local> =
        Rc::new(move |partial| {
            sink.borrow_mut().push(super::support::result_value(&partial));
            if let Some(on_update) = &on_update {
                on_update();
            }
        });
    let outcome = (tool.execute)("call".into(), input, signal, Some(update))
        .await
        .map(|result| super::support::result_value(&result))
        .map_err(|error| error.to_string());
    Called {
        outcome,
        updates: updates.take(),
    }
}

/// Renders the call row of the registered tool `name` for `args`, without control sequences.
pub fn render_call(
    runner: &ExtensionRunner<Local>,
    name: &str,
    args: &Value,
    width: usize,
) -> Vec<String> {
    let tool = registered(runner, name);
    let rows = ToolRendererState::default();
    let context = row_context(args, &rows, false);
    let view = tool
        .definition
        .renderer_binding()
        .unwrap()
        .render_call
        .expect("a call renderer")(Some(args), &dark(), &context)
    .unwrap();
    plain(&view.try_render(width).unwrap())
}

/// Renders the result row of the registered tool `name`, without control sequences.
pub fn render_result(
    runner: &ExtensionRunner<Local>,
    name: &str,
    result: &Value,
    expanded: bool,
    width: usize,
) -> Vec<String> {
    let tool = registered(runner, name);
    let args = Value::Null;
    let rows = ToolRendererState::default();
    let context = row_context(&args, &rows, false);
    let blocks: Vec<maestro_app::UserBlock> =
        serde_json::from_value(result.get("content").cloned().unwrap_or_else(|| json!([])))
            .unwrap();
    let details = result.get("details").filter(|details| !details.is_null());
    let prepared = tool
        .definition
        .renderer_binding()
        .unwrap()
        .render_result
        .expect("a result renderer")
        .prepare(maestro_app::ToolRenderResultLike {
            content: &blocks,
            details,
            terminate: None,
            is_error: Some(false),
        })
        .unwrap();
    let view = prepared
        .render(
            maestro_app::ToolRenderResultOptions {
                expanded,
                is_partial: false,
            },
            &dark(),
            &context,
        )
        .unwrap();
    plain(&view.try_render(width).unwrap())
}

/// The registered tool `name`.
fn registered(
    runner: &ExtensionRunner<Local>,
    name: &str,
) -> maestro_extensions_wasmtime::test_support::maestro_extensions::types::RegisteredTool<Local> {
    runner
        .get_all_registered_tools()
        .into_iter()
        .find(|tool| tool.definition.name() == name)
        .unwrap_or_else(|| panic!("no tool named {name}"))
}

/// The row environment of a completed call, with the shipped dark theme.
fn row_context<'a>(
    args: &'a Value,
    rows: &'a ToolRendererState,
    is_error: bool,
) -> ToolRenderContext<'a, ToolRendererState> {
    ToolRenderContext {
        args: Some(args),
        tool_call_id: "row".into(),
        invalidate: Rc::new(|| {}),
        environment: Rc::new(ToolRenderEnvironment {
            runtime: ManualRuntime::new().handle(),
            readme_path: String::new(),
            live_theme: dark(),
            highlighter: Rc::new(maestro_theme::SyntectHighlighter::new().unwrap()),
            terminal_image: TerminalImage::default(),
        }),
        last_component: None,
        state: Rc::new(RefCell::new(rows.clone())),
        renderer_state: Some(rows),
        cwd: "/work".into(),
        phase: ToolRenderPhase {
            execution_started: true,
            args_complete: true,
            is_partial: false,
        },
        expanded: false,
        show_images: false,
        is_error,
    }
}

/// A tool result message as the session stores it.
pub fn tool_result_message(id: &str, tool: &str, details: Option<Value>) -> Value {
    let mut message = json!({"role":"toolResult","toolCallId":id,"toolName":tool,"content":[],"isError":false,"timestamp":1});
    if let Some(details) = details {
        message["details"] = details;
    }
    message
}

/// Starts a new branch of `session` and appends `messages` and custom entries to it in order.
pub fn replace_branch(session: &RefCell<SessionManager>, entries: &[Value]) {
    let mut session = session.borrow_mut();
    session.reset_leaf();
    for entry in entries {
        match entry.get("customType") {
            Some(custom_type) => {
                let data = entry
                    .get("data")
                    .cloned()
                    .map_or(Presence::Missing, Presence::Present);
                session
                    .append_custom_entry(custom_type.as_str().unwrap(), data)
                    .unwrap();
            }
            None => {
                session.append_message(entry.clone()).unwrap();
            }
        }
    }
}

/// Draws the stored custom message `message` with the renderer registered for its type and
/// returns the non-empty rows, without control sequences. `None` when no renderer drew it.
pub fn render_message(
    runner: &ExtensionRunner<Local>,
    message: &Value,
    expanded: bool,
    width: usize,
) -> Option<Vec<String>> {
    let custom_type = message["customType"].as_str().unwrap();
    let renderer = runner.get_message_renderer_call::<Value>(custom_type)?;
    let view = renderer
        .call(
            Rc::new(serde_json::from_value(message.clone()).unwrap()),
            maestro_app::MessageRenderOptions { expanded },
            dark(),
        )
        .unwrap()?;
    Some(
        plain(&view.try_render(width).unwrap())
            .into_iter()
            .map(|row| row.trim().to_owned())
            .filter(|row| !row.is_empty())
            .collect(),
    )
}

/// Registers `model_id` under `provider` in the engine's session catalog, as a configured model
/// with a key.
pub fn register_model(harness: &Harness, provider: &str, model_id: &str) {
    let first = &harness.faux.models[0];
    let model = ProviderModelConfig {
        id: model_id.into(),
        name: "Configured model".into(),
        api: Some(first.api.clone()),
        base_url: Some(first.base_url.clone()),
        reasoning: first.reasoning,
        thinking_level_map: None,
        input: first.input.clone(),
        cost: first.cost.clone(),
        context_window: first.context_window,
        max_tokens: first.max_tokens,
        headers: None,
        compat: None,
    };
    let config = ProviderConfig {
        base_url: Some(first.base_url.clone()),
        api_key: Some("configured-key".into()),
        api: Some(harness.faux.api.clone()),
        models: Some(vec![model]),
        ..ProviderConfig::default()
    };
    harness
        .model_registry
        .register_provider(provider, SharedHandle::new(std::sync::RwLock::new(config)))
        .unwrap();
}
