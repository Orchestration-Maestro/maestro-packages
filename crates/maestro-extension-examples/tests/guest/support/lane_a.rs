//! Helpers the lane-a guest tests share: a terminal UI that records every request and answers
//! dialogs from a script, and a reader for styled text.
use maestro_extensions_wasmtime::test_support::maestro_extensions::{
    EventError, ExtensionRunner, ExtensionUIContext, Local, WorkingIndicatorOptions,
    footer_data_provider::{ExtensionStatuses, ReadonlyFooterDataProvider},
    types::{
        EditorFactory, EditorFactoryOutcome, ExtensionComponent, ExtensionKeybindings,
        ExtensionUIContextPort, ExtensionUIDialogOptions, ExtensionUIFuture, FooterFactory,
        NotificationType,
    },
};
use maestro_tui::{
    TUI, TerminalImage,
    test_support::{manual_runtime::ManualRuntime, recording_terminal::RecordingTerminal},
    tui::{LocalFuture, LogContext, RenderCallback, RenderTimer, TuiRuntime},
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};
use tokio::sync::Notify;

/// Every UI request an extension made, in order, and the answers its dialogs receive.
#[derive(Default)]
pub struct Ui {
    /// Each request as `{"method", "operands"}` in the host's own spelling.
    seen: RefCell<Vec<Value>>,
    /// Wakes a waiter when a request arrives.
    changed: Notify,
    /// The method whose next request fails, and the failure.
    rejection: RefCell<Option<(String, String)>>,
    /// Answers to selector dialogs, consumed in order.
    selections: RefCell<VecDeque<Option<String>>>,
    /// Answers to confirmation dialogs, consumed in order.
    confirmations: RefCell<VecDeque<bool>>,
    /// The terminal the installed components draw on, when the test supplied one.
    screen: Option<Screen>,
    /// The editor the extension installed.
    editor: RefCell<Option<EditorFactoryOutcome>>,
    /// The footer factory the extension installed.
    footer: RefCell<Option<FooterFactory>>,
}

/// A started terminal on a manual runtime whose scheduled work the test runs.
#[derive(Clone)]
pub struct Screen {
    /// The terminal factories receive.
    pub tui: TUI,
    /// The runtime whose scheduled work `settle` runs.
    pub runtime: ManualRuntime,
    /// Wakes a waiter whenever the terminal schedules a redraw.
    pub redraws: Rc<Notify>,
}

/// The manual runtime, announcing each redraw it is asked to schedule.
struct Announcing {
    /// The runtime that keeps the scheduled work.
    inner: Rc<dyn TuiRuntime>,
    /// Wakes the test.
    redraws: Rc<Notify>,
}

impl TuiRuntime for Announcing {
    fn now(&self) -> std::time::Duration {
        self.inner.now()
    }

    fn schedule(
        &self,
        delay: std::time::Duration,
        callback: RenderCallback,
    ) -> Box<dyn RenderTimer> {
        self.redraws.notify_one();
        self.inner.schedule(delay, callback)
    }

    fn spawn_local(&self, future: LocalFuture) {
        self.inner.spawn_local(future);
    }

    fn log_context(&self) -> LogContext {
        self.inner.log_context()
    }
}

/// Consumes a redraw announcement that arrived before anyone waited for it.
pub fn drain(redraws: &Notify) {
    let mut waiting = std::pin::pin!(redraws.notified());
    let _ = waiting
        .as_mut()
        .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()));
}

impl Screen {
    /// Starts an 80 by 24 terminal.
    pub fn new() -> Self {
        let terminal = RecordingTerminal::new(80, 24);
        let runtime = ManualRuntime::new();
        let redraws = Rc::new(Notify::new());
        let announcing = Announcing {
            inner: runtime.handle(),
            redraws: redraws.clone(),
        };
        let tui = TUI::new(
            terminal.handle(),
            Rc::new(announcing),
            TerminalImage::default(),
            Some(false),
        );
        tui.start().unwrap();
        runtime.settle().unwrap();
        drain(&redraws);
        Self {
            tui,
            runtime,
            redraws,
        }
    }
}

impl Ui {
    /// Installs a recording UI on `runner` and returns it.
    pub fn install(runner: &ExtensionRunner<Local>) -> Rc<Self> {
        let ui = Rc::new(Self::default());
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
        ui
    }

    /// Installs a recording UI whose editors and footers draw on a started terminal.
    pub fn install_with_screen(runner: &ExtensionRunner<Local>) -> Rc<Self> {
        let ui = Rc::new(Self {
            screen: Some(Screen::new()),
            ..Self::default()
        });
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
        ui
    }

    /// The terminal the installed components draw on.
    pub fn screen(&self) -> &Screen {
        self.screen.as_ref().expect("a UI installed with a screen")
    }

    /// The editor the extension installed, once it did.
    pub fn editor(&self) -> EditorFactoryOutcome {
        self.editor.borrow().clone().expect("an installed editor")
    }

    /// Builds the footer the extension installed over `provider`.
    pub fn build_footer(&self, provider: Rc<dyn ReadonlyFooterDataProvider>) -> ExtensionComponent {
        let factory = self.footer.borrow().clone().expect("an installed footer");
        factory
            .call((self.screen().tui.clone(), self.theme(), provider))
            .unwrap()
    }

    /// Makes the next `method` request fail with `message`.
    pub fn reject(&self, method: &str, message: &str) {
        *self.rejection.borrow_mut() = Some((method.to_owned(), message.to_owned()));
    }

    /// Queues the answer of the next selector dialog; `None` dismisses it.
    pub fn answer_select(&self, answer: Option<&str>) {
        self.selections
            .borrow_mut()
            .push_back(answer.map(str::to_owned));
    }

    /// Queues the answer of the next confirmation dialog.
    pub fn answer_confirm(&self, answer: bool) {
        self.confirmations.borrow_mut().push_back(answer);
    }

    /// Takes everything recorded since the last call.
    pub fn take(&self) -> Vec<Value> {
        std::mem::take(&mut self.seen.borrow_mut())
    }

    /// Everything recorded so far.
    pub fn seen(&self) -> Vec<Value> {
        self.seen.borrow().clone()
    }

    /// Waits until `count` requests have been recorded in total.
    pub async fn until(&self, count: usize) {
        while self.seen.borrow().len() < count {
            self.changed.notified().await;
        }
    }

    /// Records one request and applies a pending rejection of `method`.
    fn record(&self, method: &str, operands: Value) -> Result<(), EventError> {
        self.seen
            .borrow_mut()
            .push(json!({"method": method, "operands": operands}));
        self.changed.notify_one();
        let mut rejection = self.rejection.borrow_mut();
        match rejection.take_if(|(rejected, _)| rejected == method) {
            Some((_, message)) => Err(message.into()),
            None => Ok(()),
        }
    }
}

impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::from_theme(
            maestro_theme::test_support::palette::palette("dark").unwrap(),
        )
    }

    fn notify(&self, message: String, kind: Option<NotificationType>) -> Result<(), EventError> {
        self.record(
            "notify",
            json!([message, kind.map(|kind| format!("{kind:?}"))]),
        )
    }

    fn select(
        &self,
        title: String,
        options: Vec<String>,
        _opts: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<Option<String>> {
        let recorded = self.record("select", json!([title, options]));
        let answer = self.selections.borrow_mut().pop_front().flatten();
        Box::pin(async move { recorded.map(|()| answer) })
    }

    fn confirm(
        &self,
        title: String,
        message: String,
        _opts: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<bool> {
        let recorded = self.record("confirm", json!([title, message]));
        let answer = self.confirmations.borrow_mut().pop_front().unwrap_or(false);
        Box::pin(async move { recorded.map(|()| answer) })
    }

    fn set_status(&self, key: String, text: Option<String>) -> Result<(), EventError> {
        self.record("setStatus", json!([key, text]))
    }

    fn set_working_message(&self, message: Option<String>) -> Result<(), EventError> {
        self.record("setWorkingMessage", json!([message]))
    }

    fn set_working_visible(&self, visible: bool) -> Result<(), EventError> {
        self.record("setWorkingVisible", json!([visible]))
    }

    fn set_working_indicator(
        &self,
        options: Option<WorkingIndicatorOptions>,
    ) -> Result<(), EventError> {
        let operands = options
            .map(|options| json!({"frames": options.frames, "intervalMs": options.interval_ms}));
        self.record("setWorkingIndicator", json!([operands]))
    }

    fn set_title(&self, title: String) -> Result<(), EventError> {
        self.record("setTitle", json!([title]))
    }

    fn set_footer(&self, factory: Option<FooterFactory>) -> Result<(), EventError> {
        let installed = factory.is_some();
        self.footer.replace(factory);
        self.record("setFooter", json!([installed]))
    }

    fn set_editor_component(&self, factory: Option<EditorFactory>) -> Result<(), EventError> {
        let installed = factory.is_some();
        let keybindings = ExtensionKeybindings {
            manager: maestro_tui::KeybindingsManager::new(
                maestro_tui::TUI_KEYBINDINGS.clone(),
                Vec::new(),
            ),
            reload: None,
        };
        let built = match (factory, &self.screen) {
            (Some(factory), Some(screen)) => {
                let theme = maestro_theme::get_editor_theme(self.theme());
                Some(factory.call((screen.tui.clone(), theme, keybindings))?)
            }
            _ => None,
        };
        self.editor.replace(built);
        self.record("setEditorComponent", json!([installed]))
    }
}

/// Footer metadata a test controls: the branch, and the callbacks to run when it changes.
#[derive(Default)]
pub struct Metadata {
    /// The branch the footer shows.
    pub branch: RefCell<Option<String>>,
    /// The branch-change callbacks the footer registered.
    pub listeners: Rc<RefCell<Vec<Rc<dyn Fn()>>>>,
}

impl ReadonlyFooterDataProvider for Metadata {
    fn get_git_branch(&self) -> Option<String> {
        self.branch.borrow().clone()
    }

    fn get_extension_statuses(&self) -> ExtensionStatuses {
        ExtensionStatuses::default()
    }

    fn get_available_provider_count(&self) -> f64 {
        0.0
    }

    fn on_branch_change(&self, callback: Rc<dyn Fn()>) -> Box<dyn Fn()> {
        self.listeners.borrow_mut().push(callback.clone());
        let listeners = self.listeners.clone();
        Box::new(move || {
            listeners
                .borrow_mut()
                .retain(|registered| !Rc::ptr_eq(registered, &callback));
        })
    }
}

/// Removes the SGR style sequences a theme adds, leaving the text it styled.
pub fn plain(styled: &str) -> String {
    let mut text = String::new();
    let mut chars = styled.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            for end in chars.by_ref() {
                if end == 'm' {
                    break;
                }
            }
        } else {
            text.push(c);
        }
    }
    text
}

/// Builds an event record from the document an application delivers.
pub fn event<T: DeserializeOwned>(document: Value) -> T {
    serde_json::from_value(document).unwrap()
}

/// Collects the message of every callback failure the runner reports.
pub fn failures(runner: &ExtensionRunner<Local>) -> Rc<RefCell<Vec<String>>> {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let sink = seen.clone();
    let remove = runner.on_error(Rc::new(move |error| {
        sink.borrow_mut().push(error.error.clone());
        Ok(())
    }));
    std::mem::forget(remove);
    seen
}

/// A harness whose session reports `cwd`, with the code cache the run compiled.
pub fn harness_in(cwd: &str) -> maestro_extensions_wasmtime::test_support::Harness {
    use maestro_extensions_wasmtime::test_support::{Harness, HarnessOptions};
    let options = HarnessOptions {
        agent_dir: maestro_test_process::run_agent_directory().unwrap_or_default(),
        ..HarnessOptions::default()
    };
    Harness::new(cwd, options).unwrap()
}

/// Runs a test body on a local task set whose runtime has the timer and process drivers that
/// components waiting on the clock or on child processes need.
pub fn local_io<T>(test: impl Future<Output = T>) -> T {
    maestro_test_process::executor::block_on(test).unwrap()
}

/// Runs a test body on a local task set it can hand to the application's native effects.
pub fn local_native<F: Future>(test: impl FnOnce(Rc<tokio::task::LocalSet>) -> F) -> F::Output {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    local.block_on(&runtime, test(local.clone()))
}

/// Loads the component `variable` names into `harness` with the application's native effects
/// (file watches, child processes, HTTP), which the harness's own loader does not supply.
pub async fn load_native(
    harness: &maestro_extensions_wasmtime::test_support::Harness,
    variable: &str,
    local: &Rc<tokio::task::LocalSet>,
) -> maestro_extensions_wasmtime::test_support::Loaded {
    use maestro_extensions_wasmtime::test_support::{
        maestro_extensions::{
            ExtensionCommandContextActions, ExtensionExecution, ExtensionWidgets, LiveTheme,
            ModuleLoader, load_extensions_with_runtime,
            runner::ExtensionRunnerSession,
            test_support::{ToolkitWidgets, diagnostics},
            types::ProviderActions,
        },
        unconfigured_command_operations, unconfigured_context_actions,
    };
    let io = maestro_app::extension_native_io::extension_native_io(local);
    let widgets: Rc<dyn ExtensionWidgets> =
        Rc::new(ToolkitWidgets::new(LiveTheme::default()).unwrap());
    let agent_dir = maestro_test_process::run_agent_directory().unwrap_or_default();
    let loader: ModuleLoader<Local> = Rc::new(move |path: &str| {
        let factory = maestro_extensions_wasmtime::artifact_factory_with_io(
            path.to_owned(),
            widgets.clone(),
            maestro_extensions_wasmtime::Installation {
                version: env!("CARGO_PKG_VERSION").into(),
                agent_dir: agent_dir.clone(),
            },
            io.clone(),
            None,
        );
        Box::pin(std::future::ready(Ok(Some(factory))))
    });
    let loaded = load_extensions_with_runtime(
        vec![super::support::component(variable)],
        &harness.cwd,
        &harness.bus,
        &loader,
        harness.runtime.clone(),
    )
    .await
    .unwrap();
    let errors: Vec<_> = loaded.errors.iter().map(|error| &error.error).collect();
    assert!(errors.is_empty(), "{errors:?}");
    let runner = Rc::new(ExtensionRunner::<Local>::new(
        Rc::new(std::sync::RwLock::new(loaded.extensions)),
        loaded.runtime,
        ExtensionRunnerSession {
            cwd: harness.cwd.clone(),
            session_manager: harness.session_manager.clone(),
            model_registry: harness.model_registry.clone(),
        },
        Rc::new(diagnostics::write_stderr),
        LiveTheme::default(),
    ));
    runner
        .bind_core(
            Local::unbound_actions(),
            unconfigured_context_actions(),
            ProviderActions::default(),
        )
        .unwrap();
    runner.bind_command_context(Some(ExtensionCommandContextActions::new(
        unconfigured_command_operations(),
    )));
    maestro_extensions_wasmtime::test_support::Loaded {
        runner,
        errors: loaded.errors,
    }
}

/// Supports no language, so rendering never classifies code.
struct Plain;

impl maestro_theme::SyntaxHighlighter for Plain {
    fn supports_language(&self, _: &str) -> bool {
        false
    }

    fn highlight(
        &self,
        _: &str,
        _: &str,
    ) -> Result<Vec<maestro_theme::SyntaxSpan>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Vec::new())
    }
}

/// The dark theme tools render with.
fn dark() -> maestro_theme::LiveTheme {
    maestro_theme::LiveTheme::from_theme(
        maestro_theme::test_support::palette::palette("dark").unwrap(),
    )
}

/// The registered tool `name`, which must exist.
fn registered(
    runner: &ExtensionRunner<Local>,
    name: &str,
) -> maestro_extensions_wasmtime::test_support::maestro_extensions::RegisteredTool<Local> {
    runner
        .get_all_registered_tools()
        .into_iter()
        .find(|tool| tool.metadata().definition.name == name)
        .unwrap_or_else(|| panic!("no tool named {name}"))
}

/// A row's render context: completed arguments, the expansion and whether the call is partial.
fn row_context<'a>(
    args: &'a Value,
    rows: &'a maestro_extensions_wasmtime::test_support::maestro_extensions::ToolRendererState,
    expanded: bool,
    partial: bool,
) -> maestro_extensions_wasmtime::test_support::maestro_extensions::ToolRenderContext<
    'a,
    maestro_extensions_wasmtime::test_support::maestro_extensions::ToolRendererState,
> {
    use maestro_extensions_wasmtime::test_support::maestro_extensions::{
        ToolRenderContext, ToolRenderEnvironment, ToolRenderPhase,
    };
    let environment = Rc::new(ToolRenderEnvironment {
        runtime: ManualRuntime::new().handle(),
        readme_path: String::new(),
        live_theme: maestro_theme::LiveTheme::default(),
        highlighter: Rc::new(Plain),
        terminal_image: TerminalImage::default(),
    });
    ToolRenderContext {
        args: Some(args),
        tool_call_id: "call".into(),
        invalidate: Rc::new(|| {}),
        environment,
        last_component: None,
        state: Rc::new(RefCell::new(rows.clone())),
        renderer_state: Some(rows),
        cwd: "/work".into(),
        phase: ToolRenderPhase {
            execution_started: true,
            args_complete: true,
            is_partial: partial,
        },
        expanded,
        show_images: false,
        is_error: false,
    }
}

/// The unstyled rows, without their trailing padding, the registered tool `name` draws for a
/// call with `args`.
pub fn render_call(runner: &ExtensionRunner<Local>, name: &str, args: &Value) -> Vec<String> {
    use maestro_tui::tui::Component;
    let rows =
        maestro_extensions_wasmtime::test_support::maestro_extensions::ToolRendererState::default();
    let context = row_context(args, &rows, false, false);
    registered(runner, name)
        .renderer_binding()
        .unwrap()
        .render_call
        .unwrap()(Some(args), &dark(), &context)
    .unwrap()
    .try_render(200)
    .unwrap()
    .iter()
    .map(|row| plain(row).trim_end().to_owned())
    .collect()
}

/// The unstyled rows the registered tool `name` draws for a result in the given state.
pub fn render_result(
    runner: &ExtensionRunner<Local>,
    name: &str,
    args: &Value,
    result: &Value,
    flags: [bool; 2],
) -> Vec<String> {
    use maestro_extensions_wasmtime::test_support::maestro_extensions::{
        ToolRenderResultLike, ToolRenderResultOptions, ToolRendererState,
    };
    use maestro_tui::tui::Component;
    let [expanded, partial] = flags;
    let content: Vec<_> = serde_json::from_value(result["content"].clone()).unwrap();
    let like = ToolRenderResultLike {
        content: &content,
        details: result.get("details"),
        terminate: None,
        is_error: Some(false),
    };
    let rows = ToolRendererState::default();
    let context = row_context(args, &rows, expanded, partial);
    registered(runner, name)
        .renderer_binding()
        .unwrap()
        .render_result
        .unwrap()
        .prepare(like)
        .unwrap()
        .render(
            ToolRenderResultOptions {
                expanded,
                is_partial: partial,
            },
            &dark(),
            &context,
        )
        .unwrap()
        .try_render(200)
        .unwrap()
        .iter()
        .map(|row| plain(row).trim_end().to_owned())
        .collect()
}
