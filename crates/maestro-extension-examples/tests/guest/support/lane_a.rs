//! Helpers the lane-a guest tests share: a terminal UI that records every request and answers
//! dialogs from a script, and a reader for styled text.
use maestro_extensions_wasmtime::test_support::maestro_extensions::{
    EventError, ExtensionRunner, ExtensionUIContext, Local, WorkingIndicatorOptions,
    types::{
        ExtensionUIContextPort, ExtensionUIDialogOptions, ExtensionUIFuture, NotificationType,
    },
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
}

impl Ui {
    /// Installs a recording UI on `runner` and returns it.
    pub fn install(runner: &ExtensionRunner<Local>) -> Rc<Self> {
        let ui = Rc::new(Self::default());
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
        ui
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
