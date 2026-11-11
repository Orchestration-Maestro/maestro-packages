//! The terminal UI an extension talks to in the guest tests: it records every request, answers
//! dialogs from a script, refuses a request on demand and builds the components it is given.
mod components;

use super::rows::dark;
use maestro_extensions::{
    EventError, ExtensionRunner, ExtensionUIContext, Local, WorkingIndicatorOptions,
    footer::ReadonlyFooterDataProvider,
    types::{
        AutocompleteProviderFactory, ComponentFactory, CustomComponentFactory, CustomUIOptions,
        EditorFactory, EditorFactoryOutcome, ExtensionComponent, ExtensionKeybindings,
        ExtensionUIContextPort, ExtensionUIDialogOptions, ExtensionUIFuture,
        ExtensionWidgetOptions, FooterFactory, NotificationType, WidgetContent,
    },
};
use maestro_theme::LiveTheme;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
};
use tokio::sync::Notify;

pub use crate::terminal::{Stage, drain};

/// Which request the UI refuses and the host error it fails with.
enum Refusal {
    /// The next request of this method.
    Method(String, String),
    /// The request at this index, counted from zero since the last `take`.
    Index(usize, String),
}

/// What the UI records when the application asks it to show a component.
#[derive(Default)]
struct Views {
    /// The inputs the next component receives, when the test scripted them.
    script: RefCell<Option<Vec<String>>>,
    /// The rows of the scripted component before the first input and after each one.
    frames: Rc<RefCell<Vec<Vec<String>>>>,
    /// The width the scripted component is drawn at.
    width: Cell<usize>,
    /// The components shown and kept open, oldest first.
    shown: RefCell<Vec<Rc<ExtensionComponent>>>,
    /// Wakes a waiter when a component is shown.
    opened: Notify,
}

/// Every request an extension made of the application's UI, in order, and the scripted answers
/// and refusals the next requests meet.
pub struct Recorder {
    /// Each request as `[method, operands]`, with the method in the host's camel case.
    calls: RefCell<Vec<Value>>,
    /// The pending refusal.
    refusal: RefCell<Option<Refusal>>,
    /// Answers the dialogs give, oldest first.
    answers: RefCell<VecDeque<Value>>,
    /// The answer a dialog gives when no queued answer is left; null dismisses it.
    reply: RefCell<Value>,
    /// Wakes a waiter when a request arrives.
    requested: Notify,
    /// What runs once, right after the next request is recorded.
    after_first: RefCell<Option<Box<dyn FnOnce()>>>,
    /// The theme the UI publishes.
    theme: LiveTheme,
    /// The terminal that components are built for, when the UI has one.
    stage: Option<Stage>,
    /// The components the extension asked to show.
    views: Views,
    /// The editor the extension's factory last built, when it set one.
    editor: RefCell<Option<EditorFactoryOutcome>>,
    /// The footer factory the extension installed.
    footer: RefCell<Option<FooterFactory>>,
    /// The header factory the extension installed.
    header: RefCell<Option<ComponentFactory>>,
}

/// The size of the terminal that [`Recorder::install_on`] starts.
pub const TERMINAL: (usize, usize) = (80, 24);

impl Recorder {
    /// Installs a recording UI without a terminal on `runner` and returns it.
    pub fn install(runner: &ExtensionRunner<Local>) -> Rc<Self> {
        Self::install_with(runner, None)
    }

    /// Installs a recording UI whose components are built for a started terminal of `columns`
    /// by `rows`.
    pub fn install_on(
        runner: &ExtensionRunner<Local>,
        (columns, rows): (usize, usize),
    ) -> Rc<Self> {
        Self::install_with(runner, Some(Stage::new(columns, rows)))
    }

    /// Installs the UI over an optional terminal.
    fn install_with(runner: &ExtensionRunner<Local>, stage: Option<Stage>) -> Rc<Self> {
        let ui = Self::detached(stage);
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
        ui
    }

    /// A recording UI that no runner is connected to yet; see [`Recorder::connect`].
    pub fn detached(stage: Option<Stage>) -> Rc<Self> {
        Rc::new(Self {
            calls: RefCell::default(),
            refusal: RefCell::default(),
            answers: RefCell::default(),
            reply: RefCell::new(Value::Null),
            requested: Notify::new(),
            after_first: RefCell::default(),
            theme: dark(),
            stage,
            views: Views {
                width: Cell::new(TERMINAL.0),
                ..Views::default()
            },
            editor: RefCell::default(),
            footer: RefCell::default(),
            header: RefCell::default(),
        })
    }

    /// Connects `runner` to this UI, or disconnects it so the runner is headless.
    pub fn connect(self: &Rc<Self>, runner: &ExtensionRunner<Local>, connected: bool) {
        runner.set_ui_context(connected.then(|| ExtensionUIContext::new(self.clone())));
    }

    /// The terminal that components are built for.
    pub fn stage(&self) -> &Stage {
        self.stage.as_ref().expect("a UI installed with a terminal")
    }

    /// Queues the answer the next dialog gives.
    pub fn answer(&self, answer: Value) {
        self.answers.borrow_mut().push_back(answer);
    }

    /// Sets the answer every dialog gives once the queued answers are used.
    pub fn reply(&self, reply: Value) {
        *self.reply.borrow_mut() = reply;
    }

    /// Makes the next request of `method` fail with `message`.
    pub fn reject(&self, method: &str, message: &str) {
        *self.refusal.borrow_mut() = Some(Refusal::Method(method.into(), message.into()));
    }

    /// Makes the request at `index` fail with `message`.
    pub fn reject_at(&self, index: usize, message: &str) {
        *self.refusal.borrow_mut() = Some(Refusal::Index(index, message.into()));
    }

    /// Drops a refusal no request met.
    pub fn disarm(&self) {
        self.refusal.borrow_mut().take();
    }

    /// Runs `action` once, right after the next request is recorded.
    pub fn after_first_request(&self, action: impl FnOnce() + 'static) {
        *self.after_first.borrow_mut() = Some(Box::new(action));
    }

    /// Takes the requests recorded since the last call.
    pub fn take(&self) -> Vec<Value> {
        std::mem::take(&mut self.calls.borrow_mut())
    }

    /// Everything recorded since the last `take`.
    pub fn seen(&self) -> Vec<Value> {
        self.calls.borrow().clone()
    }

    /// Waits for the frame writer to announce a redraw, then runs the work it scheduled.
    pub async fn redrawn(&self) {
        let stage = self.stage();
        stage.redraws.notified().await;
        stage.settle();
    }

    /// Waits until `count` requests have been recorded since the last `take`.
    pub async fn until(&self, count: usize) {
        while self.calls.borrow().len() < count {
            self.requested.notified().await;
        }
    }

    /// The editor the extension's factory built, if any.
    pub fn editor(&self) -> Option<EditorFactoryOutcome> {
        self.editor.borrow().clone()
    }

    /// The rows the installed editor renders `width` cells wide, without control sequences.
    pub fn editor_rows(&self, width: usize) -> Vec<String> {
        let built = self.editor().expect("an installed editor");
        super::rows::plain_rows(&built.editor().try_render(width).unwrap())
    }

    /// The header factory the extension installed, if any.
    pub fn header(&self) -> Option<ComponentFactory> {
        self.header.borrow().clone()
    }

    /// Builds the footer the extension installed over `provider`.
    pub fn build_footer(&self, provider: Rc<dyn ReadonlyFooterDataProvider>) -> ExtensionComponent {
        let factory = self.footer.borrow().clone().expect("an installed footer");
        factory
            .call((self.stage().tui.clone(), self.theme(), provider))
            .unwrap()
    }

    /// Runs the work the frame writer scheduled.
    pub fn settle(&self) {
        if let Some(stage) = &self.stage {
            stage.settle();
        }
    }

    /// Records one request, from the UI or from a host operation the test serves, and applies
    /// the pending refusal to it.
    pub fn record(&self, method: &str, operands: Value) -> Result<(), EventError> {
        let index = self.calls.borrow().len();
        self.calls
            .borrow_mut()
            .push(Value::Array(vec![method.into(), operands]));
        self.requested.notify_one();
        let hook = self.after_first.borrow_mut().take();
        if let Some(hook) = hook {
            hook();
        }
        let mut refusal = self.refusal.borrow_mut();
        let message = match &*refusal {
            Some(Refusal::Method(wanted, message)) if wanted == method => message.clone(),
            Some(Refusal::Index(wanted, message)) if *wanted == index => message.clone(),
            _ => return Ok(()),
        };
        *refusal = None;
        Err(message.into())
    }

    /// Records a dialog and settles it with the next queued answer, or the standing reply.
    fn dialog(
        &self,
        method: &str,
        mut operands: Vec<Value>,
        options: Option<&ExtensionUIDialogOptions>,
    ) -> Result<Value, EventError> {
        let signal = options.and_then(|options| options.signal.as_ref());
        let aborted = signal.map_or(Value::Null, |signal| json!(signal.is_aborted()));
        let timeout = options
            .and_then(|options| options.timeout)
            .map_or(Value::Null, |timeout| json!(timeout));
        operands.extend([aborted, timeout]);
        self.record(method, Value::Array(operands))?;
        let queued = self.answers.borrow_mut().pop_front();
        Ok(queued.unwrap_or_else(|| self.reply.borrow().clone()))
    }
}

/// A notification severity in the host's own spelling.
fn kind(kind: Option<NotificationType>) -> Value {
    json!(kind.map(|kind| format!("{kind:?}")))
}

/// The key bindings every built component receives.
pub(super) fn keybindings() -> ExtensionKeybindings {
    ExtensionKeybindings {
        manager: (*maestro_app::KeybindingsManager::new(Vec::new(), None)).clone(),
        reload: None,
    }
}

impl ExtensionUIContextPort for Recorder {
    fn theme(&self) -> LiveTheme {
        self.theme.clone()
    }

    fn select(
        &self,
        title: String,
        options: Vec<String>,
        opts: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<Option<String>> {
        let answer = self
            .dialog("select", vec![json!(title), json!(options)], opts.as_ref())
            .map(|answer| answer.as_str().map(str::to_owned));
        Box::pin(std::future::ready(answer))
    }

    fn confirm(
        &self,
        title: String,
        message: String,
        opts: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<bool> {
        let answer = self
            .dialog("confirm", vec![json!(title), json!(message)], opts.as_ref())
            .map(|answer| answer == true);
        Box::pin(std::future::ready(answer))
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

    fn set_hidden_thinking_label(&self, label: Option<String>) -> Result<(), EventError> {
        self.record("setHiddenThinkingLabel", json!([label]))
    }

    fn set_title(&self, title: String) -> Result<(), EventError> {
        self.record("setTitle", json!([title]))
    }

    fn set_widget(
        &self,
        key: String,
        content: Option<WidgetContent>,
        options: Option<ExtensionWidgetOptions>,
    ) -> Result<(), EventError> {
        let lines = match content {
            Some(WidgetContent::Lines(lines)) => json!(lines),
            Some(WidgetContent::Factory(_)) => json!("factory"),
            None => Value::Null,
        };
        let placement = options
            .and_then(|options| options.placement)
            .map(|placement| format!("{placement:?}"));
        self.record("setWidget", json!([key, lines, placement]))
    }

    fn add_autocomplete_provider(&self, _: AutocompleteProviderFactory) -> Result<(), EventError> {
        self.record("addAutocompleteProvider", json!([]))
    }

    fn set_header(&self, factory: Option<ComponentFactory>) -> Result<(), EventError> {
        self.record("setHeader", json!([factory.is_some()]))?;
        *self.header.borrow_mut() = factory;
        Ok(())
    }

    fn set_footer(&self, factory: Option<FooterFactory>) -> Result<(), EventError> {
        self.record("setFooter", json!([factory.is_some()]))?;
        self.footer.replace(factory);
        Ok(())
    }

    fn set_editor_component(&self, factory: Option<EditorFactory>) -> Result<(), EventError> {
        self.record("setEditorComponent", json!([factory.is_some()]))?;
        let Some(stage) = &self.stage else {
            return Ok(());
        };
        let theme = maestro_theme::get_editor_theme(self.theme());
        let built = factory
            .map(|factory| factory.call((stage.tui.clone(), theme, keybindings())))
            .transpose()?;
        self.editor.replace(built);
        Ok(())
    }

    fn custom_component(
        &self,
        factory: CustomComponentFactory,
        _: Option<CustomUIOptions>,
    ) -> ExtensionUIFuture<()> {
        self.show(&factory)
    }
}
