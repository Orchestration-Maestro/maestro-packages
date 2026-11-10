//! Helpers the lane-a guest tests share: a terminal UI that records every request and answers
//! dialogs from a script, and a reader for styled text.
use maestro_extensions_wasmtime::test_support::maestro_extensions::{
    EventError, ExtensionRunner, ExtensionUIContext, Local, WorkingIndicatorOptions,
    footer_data_provider::{ExtensionStatuses, ReadonlyFooterDataProvider},
    types::{
        CustomClose, CustomComponentFactory, CustomFactoryOutcome, CustomUIOptions, EditorFactory,
        EditorFactoryOutcome, ExtensionComponent, ExtensionKeybindings, ExtensionUIContextPort,
        ExtensionUIDialogOptions, ExtensionUIFuture, FooterFactory, NotificationType,
    },
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
};
use tokio::sync::Notify;

/// The width custom components are drawn at.
const CUSTOM_WIDTH: usize = 20;

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
    /// The key presses a custom component receives, in order.
    presses: RefCell<VecDeque<String>>,
    /// The rows of the custom component before the first press and after each one.
    frames: Rc<RefCell<Vec<Vec<String>>>>,
}

#[path = "lane_a/native.rs"]
mod native;
#[path = "lane_a/render.rs"]
mod render;
#[path = "lane_a/screen.rs"]
mod screen;
pub use native::{load_native, local_native};
pub use render::{render_call, render_result};
pub use screen::{Screen, drain};

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

    /// Queues the terminal input the next custom component receives, one entry per press.
    pub fn press(&self, input: &str) {
        self.presses.borrow_mut().push_back(input.to_owned());
    }

    /// The rows of the last custom component: before the first press and after each press.
    pub fn frames(&self) -> Vec<Vec<String>> {
        self.frames.take()
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
        let request = serde_json::Map::from_iter([
            ("method".to_owned(), Value::from(method)),
            ("operands".to_owned(), operands),
        ]);
        self.seen.borrow_mut().push(Value::Object(request));
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

    fn custom_component(
        &self,
        factory: CustomComponentFactory,
        _options: Option<CustomUIOptions>,
    ) -> ExtensionUIFuture<()> {
        let closed = Rc::new(Cell::new(false));
        let flag = closed.clone();
        let close: CustomClose = Rc::new(move || Ok(!flag.replace(true)));
        let keybindings = ExtensionKeybindings {
            manager: maestro_tui::KeybindingsManager::new(
                maestro_tui::TUI_KEYBINDINGS.clone(),
                Vec::new(),
            ),
            reload: None,
        };
        let built = factory.call((self.screen().tui.clone(), self.theme(), keybindings, close));
        let presses = self.presses.take();
        let frames = self.frames.clone();
        Box::pin(drive(built, presses, closed, frames))
    }

    fn add_autocomplete_provider(
        &self,
        _factory: maestro_extensions_wasmtime::test_support::maestro_extensions::types::AutocompleteProviderFactory,
    ) -> Result<(), EventError> {
        self.record("addAutocompleteProvider", json!([]))
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

/// A callback the footer registered for branch changes.
pub type Callback = Rc<dyn Fn()>;

/// Footer metadata a test controls: the branch, and the callbacks to run when it changes.
#[derive(Default)]
pub struct Metadata {
    /// The branch the footer shows.
    pub branch: RefCell<Option<String>>,
    /// The branch-change callbacks the footer registered.
    pub listeners: Rc<RefCell<Vec<Callback>>>,
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

/// Opens the built custom component, draws it before and after each press and disposes it.
async fn drive(
    built: Result<CustomFactoryOutcome, EventError>,
    presses: VecDeque<String>,
    closed: Rc<Cell<bool>>,
    frames: Rc<RefCell<Vec<Vec<String>>>>,
) -> Result<(), EventError> {
    let component = match built? {
        CustomFactoryOutcome::Ready(component) => component,
        CustomFactoryOutcome::Async(pending) => pending.await?,
    };
    let handle = component.component.clone();
    let draw = || {
        handle
            .try_render(CUSTOM_WIDTH)
            .map_err(|error| error.to_string())
    };
    frames.borrow_mut().push(draw()?);
    for press in presses.iter().take_while(|_| !closed.get()) {
        if let Some(input) = handle.input_handler() {
            input
                .try_handle_input(press)
                .map_err(|error| error.to_string())?;
        }
        frames.borrow_mut().push(draw()?);
    }
    component.dispose.map_or(Ok(()), |dispose| dispose.call(()))
}

/// Removes the SGR style sequences a theme adds, leaving the text it styled.
pub fn plain(styled: &str) -> String {
    let mut text = String::new();
    let mut chars = styled.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            chars.by_ref().any(|end| end == 'm');
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
