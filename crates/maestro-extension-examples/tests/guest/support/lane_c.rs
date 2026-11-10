//! Recording UI, scripted dialog answers, error capture and typed event delivery for the
//! guest tests that script a user interface.
use maestro_extensions::runner::RunnerEmitEvent;
use maestro_extensions::widgets::ExtensionKeybindings;
use maestro_extensions::{
    EventError, ExtensionActions, ExtensionContextActions, ExtensionExecution, ExtensionRunner,
    ExtensionUIContext, Local, ToolCallEvent, ToolRenderContext, ToolRenderEnvironment,
    ToolRenderPhase, ToolRenderResultLike, ToolRenderResultOptions, ToolRendererState,
    types::{
        ComponentFactory, CustomComponentFactory, CustomFactoryOutcome, EditorFactory,
        EditorFactoryOutcome, ExtensionUIContextPort, ExtensionUIDialogOptions, ExtensionUIFuture,
        ExtensionWidgetOptions, NotificationType, ProviderActions, WidgetContent,
    },
};
use maestro_extensions_wasmtime::test_support::unconfigured_context_actions;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    future::ready,
    rc::Rc,
};

/// Which request the UI refuses and the host error it fails with.
enum Refusal {
    /// The next request of this method.
    Method(String, String),
    /// The request at this index, counted from zero since the last `take`.
    Index(usize, String),
}

/// A terminal UI that records every request as `[method, operands]`, answers dialogs from a
/// scripted value and can refuse a request.
#[derive(Default)]
pub struct Dialogs {
    /// Requests since the last `take`, in arrival order.
    seen: RefCell<Vec<Value>>,
    /// The value every dialog answers with.
    reply: RefCell<Value>,
    /// The pending refusal.
    refusal: RefCell<Option<Refusal>>,
    /// The inputs the next custom component is given, in order.
    script: RefCell<Vec<String>>,
    /// The rows of each frame the last custom component drew, with styling removed.
    frames: RefCell<Vec<Vec<String>>>,
    /// The editor the extension's factory built, if any.
    editor: RefCell<Option<EditorFactoryOutcome>>,
    /// The header factory the extension installed, if any.
    header: RefCell<Option<ComponentFactory>>,
    /// What runs once, right after the first request is recorded.
    after_first: RefCell<Option<Box<dyn FnOnce()>>>,
}

impl Dialogs {
    /// Installs a recording UI on `runner` and returns it.
    pub fn install(runner: &ExtensionRunner<Local>) -> Rc<Self> {
        let ui = Rc::new(Self::default());
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
        ui
    }

    /// Connects `runner` to this UI, or disconnects it so the runner is headless.
    pub fn connect(self: &Rc<Self>, runner: &ExtensionRunner<Local>, connected: bool) {
        runner.set_ui_context(connected.then(|| ExtensionUIContext::new(self.clone())));
    }

    /// Sets the answer of the dialogs that follow.
    pub fn answer(&self, reply: Value) {
        *self.reply.borrow_mut() = reply;
    }

    /// Makes the next request of `method` fail with `message`.
    pub fn refuse_method(&self, method: &str, message: &str) {
        *self.refusal.borrow_mut() = Some(Refusal::Method(method.into(), message.into()));
    }

    /// Makes the request at `index` fail with `message`.
    pub fn refuse_at(&self, index: usize, message: &str) {
        *self.refusal.borrow_mut() = Some(Refusal::Index(index, message.into()));
    }

    /// Runs `action` once, right after the next request is recorded.
    pub fn after_first_request(&self, action: impl FnOnce() + 'static) {
        *self.after_first.borrow_mut() = Some(Box::new(action));
    }

    /// Gives the next custom component these inputs, in order.
    pub fn script(&self, inputs: &[&str]) {
        *self.script.borrow_mut() = inputs.iter().map(|input| (*input).to_owned()).collect();
    }

    /// The rows of each frame the last custom component drew.
    pub fn frames(&self) -> Vec<Vec<String>> {
        std::mem::take(&mut self.frames.borrow_mut())
    }

    /// The editor the extension's factory built, if any.
    pub fn editor(&self) -> Option<EditorFactoryOutcome> {
        self.editor.borrow().clone()
    }

    /// The header factory the extension installed, if any.
    pub fn header(&self) -> Option<ComponentFactory> {
        self.header.borrow().clone()
    }

    /// Drops a refusal no request used.
    pub fn clear_refusal(&self) {
        *self.refusal.borrow_mut() = None;
    }

    /// Takes the requests recorded since the last call.
    pub fn take(&self) -> Vec<Value> {
        std::mem::take(&mut self.seen.borrow_mut())
    }

    /// Records one request, from the UI or from a host operation the test serves, and applies
    /// the pending refusal to it.
    pub fn request(&self, method: &str, operands: Value) -> Result<(), EventError> {
        let index = self.seen.borrow().len();
        self.seen
            .borrow_mut()
            .push(Value::Array(vec![method.into(), operands]));
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

    /// Builds the custom component, gives it the scripted inputs until it closes and records a
    /// frame before the first input and after each.
    fn show(&self, factory: &CustomComponentFactory) -> Result<(), EventError> {
        self.request("custom", json!([]))?;
        let closed = Rc::new(Cell::new(false));
        let done = closed.clone();
        let keys = ExtensionKeybindings {
            manager: (*maestro_app::KeybindingsManager::new(Vec::new(), None)).clone(),
            reload: None,
        };
        let outcome = factory.call((
            screen(),
            self.theme(),
            keys,
            Rc::new(move || Ok(!done.replace(true))),
        ))?;
        let CustomFactoryOutcome::Ready(built) = outcome else {
            return Err("the custom component was not ready".into());
        };
        let mut frames = Vec::new();
        let mut frame =
            |built: &maestro_extensions::types::ExtensionComponent| -> Result<(), EventError> {
                let rows = built
                    .component
                    .try_render(80)
                    .map_err(|error| error.to_string())?;
                frames.push(
                    rows.iter()
                        .map(|row| maestro_app::strip_ansi(row).unwrap())
                        .collect(),
                );
                Ok(())
            };
        frame(&built)?;
        let inputs = std::mem::take(&mut *self.script.borrow_mut());
        for input in inputs {
            if closed.get() {
                break;
            }
            let handler = built
                .component
                .input_handler()
                .ok_or("the component takes no input")?;
            handler
                .try_handle_input(&input)
                .map_err(|error| error.to_string())?;
            frame(&built)?;
        }
        *self.frames.borrow_mut() = frames;
        Ok(())
    }

    /// Records a dialog and answers with the scripted value.
    fn dialog(
        &self,
        method: &str,
        mut operands: Vec<Value>,
        options: Option<&ExtensionUIDialogOptions>,
    ) -> Result<Value, EventError> {
        let signal = options.as_ref().and_then(|options| options.signal.as_ref());
        let recorded = signal.map_or(Value::Null, |signal| json!(signal.is_aborted()));
        let timeout = options
            .as_ref()
            .and_then(|options| options.timeout)
            .map_or(Value::Null, |timeout| json!(timeout));
        operands.extend([recorded, timeout]);
        self.request(method, Value::Array(operands))?;
        Ok(self.reply.borrow().clone())
    }
}

impl ExtensionUIContextPort for Dialogs {
    fn theme(&self) -> maestro_theme::LiveTheme {
        dark()
    }

    fn custom_component(
        &self,
        factory: CustomComponentFactory,
        _: Option<maestro_extensions::types::CustomUIOptions>,
    ) -> ExtensionUIFuture<()> {
        Box::pin(ready(self.show(&factory)))
    }

    fn set_editor_component(&self, factory: Option<EditorFactory>) -> Result<(), EventError> {
        self.request("setEditorComponent", json!([factory.is_some()]))?;
        let keybindings = ExtensionKeybindings {
            manager: (*maestro_app::KeybindingsManager::new(Vec::new(), None)).clone(),
            reload: None,
        };
        let theme = maestro_theme::get_editor_theme(self.theme());
        let built = factory
            .map(|factory| factory.call((screen(), theme, keybindings)))
            .transpose()?;
        *self.editor.borrow_mut() = built;
        Ok(())
    }

    fn select(
        &self,
        title: String,
        options: Vec<String>,
        opts: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<Option<String>> {
        let reply = self
            .dialog("select", vec![json!(title), json!(options)], opts.as_ref())
            .map(|reply| reply.as_str().map(str::to_owned));
        Box::pin(ready(reply))
    }

    fn confirm(
        &self,
        title: String,
        message: String,
        opts: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<bool> {
        let reply = self
            .dialog("confirm", vec![json!(title), json!(message)], opts.as_ref())
            .map(|reply| reply == true);
        Box::pin(ready(reply))
    }

    fn notify(&self, message: String, kind: Option<NotificationType>) -> Result<(), EventError> {
        self.request(
            "notify",
            json!([message, kind.map(|kind| format!("{kind:?}"))]),
        )
    }

    fn set_header(&self, factory: Option<ComponentFactory>) -> Result<(), EventError> {
        self.request("setHeader", json!([factory.is_some()]))?;
        *self.header.borrow_mut() = factory;
        Ok(())
    }

    fn set_status(&self, key: String, text: Option<String>) -> Result<(), EventError> {
        self.request("setStatus", json!([key, text]))
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
        self.request("setWidget", json!([key, lines, placement]))
    }
}

/// The attributed failures a runner reported for callbacks whose errors do not propagate.
pub struct Failures {
    /// Failure messages in report order.
    seen: Rc<RefCell<Vec<String>>>,
    /// Removes the listener when the collector goes away.
    stop: Box<dyn Fn()>,
}

impl Failures {
    /// Starts collecting the failures `runner` reports.
    pub fn collect(runner: &ExtensionRunner<Local>) -> Self {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let sink = seen.clone();
        let stop = runner.on_error(Rc::new(move |error| {
            sink.borrow_mut().push(error.error.clone());
            Ok(())
        }));
        Self {
            seen,
            stop: Box::new(stop),
        }
    }

    /// Takes the failure messages reported since the last call.
    pub fn take(&self) -> Vec<String> {
        std::mem::take(&mut self.seen.borrow_mut())
    }
}

/// Delivers `event`, a recorded event document, to the handlers of its kind as the application
/// does and returns the verdict they gave, or null when none did.
pub async fn emit<V>(runner: &ExtensionRunner<Local>, event: &Value) -> Result<Value, String>
where
    V: RunnerEmitEvent + DeserializeOwned,
    V::Result: Serialize,
{
    let mut event: V = serde_json::from_value(event.clone()).map_err(|error| error.to_string())?;
    let verdict = runner
        .emit(&mut event)
        .await
        .map_err(|error| error.to_string())?;
    Ok(verdict.map_or(Value::Null, |verdict| json!(verdict)))
}

/// Delivers a recorded tool call to the hooks as the application does and returns the verdict,
/// or null when none was given.
pub async fn tool_call(runner: &ExtensionRunner<Local>, event: &Value) -> Result<Value, String> {
    let mut event: ToolCallEvent =
        serde_json::from_value(event.clone()).map_err(|error| error.to_string())?;
    let verdict = runner
        .emit_tool_call(&mut event)
        .await
        .map_err(|error| error.to_string())?;
    Ok(verdict.map_or(Value::Null, |verdict| json!(verdict)))
}

impl Drop for Failures {
    fn drop(&mut self) {
        (self.stop)();
    }
}

/// Binds `runner` to application and context operations that start from the ones nothing
/// serves and are then adjusted by `serve`.
pub fn bind(
    runner: &ExtensionRunner<Local>,
    serve: impl FnOnce(&mut ExtensionActions<Local>, &mut ExtensionContextActions),
) {
    let mut actions = Local::unbound_actions();
    let mut context = unconfigured_context_actions();
    serve(&mut actions, &mut context);
    runner
        .bind_core(actions, context, ProviderActions::default())
        .unwrap();
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

/// A result row's render context: completed arguments, no images and a redraw that does nothing.
fn row_context<'a>(
    args: &'a Value,
    rows: &'a ToolRendererState,
    expanded: bool,
) -> ToolRenderContext<'a, ToolRendererState> {
    let environment = Rc::new(ToolRenderEnvironment {
        runtime: maestro_tui::test_support::manual_runtime::ManualRuntime::new().handle(),
        readme_path: String::new(),
        live_theme: dark(),
        highlighter: Rc::new(Plain),
        terminal_image: maestro_tui::TerminalImage::default(),
    });
    ToolRenderContext {
        args: Some(args),
        tool_call_id: "row".into(),
        invalidate: Rc::new(|| {}),
        environment,
        last_component: None,
        state: Rc::new(RefCell::new(rows.clone())),
        renderer_state: Some(rows),
        cwd: "/work".into(),
        phase: ToolRenderPhase {
            execution_started: true,
            args_complete: true,
            is_partial: false,
        },
        expanded,
        show_images: false,
        is_error: false,
    }
}

/// The text rows the registered result renderer of `tool` draws for a result with `content` and
/// `details`, at `width`, with terminal styling removed and trailing padding trimmed.
pub fn result_rows(
    runner: &ExtensionRunner<Local>,
    tool: &str,
    result: (&Value, Option<&Value>),
    expanded: bool,
    width: usize,
) -> Vec<String> {
    let registered = runner
        .get_all_registered_tools()
        .into_iter()
        .find(|registered| registered.definition.name() == tool)
        .unwrap_or_else(|| panic!("no tool named {tool}"));
    let (content, details) = result;
    let content: Vec<_> = serde_json::from_value(content.clone()).unwrap();
    let rows = ToolRendererState::default();
    let args = json!({});
    let row = row_context(&args, &rows, expanded);
    let rendered = registered
        .definition
        .renderer_binding()
        .unwrap()
        .render_result
        .unwrap()
        .prepare(ToolRenderResultLike {
            content: &content,
            details,
            terminate: None,
            is_error: Some(false),
        })
        .unwrap()
        .render(
            ToolRenderResultOptions {
                expanded,
                is_partial: false,
            },
            &dark(),
            &row,
        )
        .unwrap()
        .try_render(width)
        .unwrap();
    rendered
        .iter()
        .map(|row| maestro_app::strip_ansi(row).unwrap().trim_end().to_owned())
        .collect()
}

/// A terminal writer over a recording terminal and a manual runtime, for components that need
/// one to be built.
pub fn screen() -> maestro_tui::TUI {
    let terminal = maestro_tui::test_support::recording_terminal::RecordingTerminal::new(40, 12);
    maestro_tui::TUI::new(
        terminal.handle(),
        maestro_tui::test_support::manual_runtime::ManualRuntime::new().handle(),
        maestro_tui::TerminalImage::default(),
        Some(false),
    )
}

/// The shipped dark palette, published for components that read the live theme.
pub fn dark() -> maestro_theme::LiveTheme {
    maestro_theme::LiveTheme::from_theme(
        maestro_theme::test_support::palette::palette("dark").unwrap(),
    )
}

/// Fails with the message in `slot`, if it holds one.
pub fn fail_with(slot: &RefCell<Option<String>>) -> Result<(), EventError> {
    match slot.borrow().clone() {
        Some(message) => Err(message.into()),
        None => Ok(()),
    }
}

/// Runs `test` to completion on a current-thread runtime with timers, inside a local task set,
/// for components that create a timer.
pub fn local_timed<T>(test: impl Future<Output = T>) -> T {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, test)
}
