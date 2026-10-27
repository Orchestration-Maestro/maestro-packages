//! Behavioral checks at the registered command and listener boundary.
use std::{cell::RefCell, collections::BTreeMap, future::Future, rc::Rc, task::{Context, Poll, Waker}};
use maestro_extensions_wasm::*;
use serde_json::{Value, json};
use super::register_autonomy;
use crate::autonomy;

/// Host capturing registrations and emitted payloads without running network work.
#[derive(Default)]
struct Host {
    /// Registered handlers, invoked by the tests.
    commands: RefCell<BTreeMap<String, CommandOptions>>,
    /// Registered trigger listener.
    listener: RefCell<Option<EventListener>>,
    /// Payloads delivered through the registered listener boundary.
    payloads: RefCell<Vec<Value>>,
}
impl ExtensionHost for Rc<Host> {
    fn on(&self, _event: &str, _handler: ExtensionHandler) -> ExtensionResult<()> { Err("unused capability".into()) }
    fn register_command(&self, name: &str, options: CommandOptions) -> ExtensionResult<()> { self.commands.borrow_mut().insert(name.into(), options); Ok(()) }
    fn register_shortcut(&self, _shortcut: &str, _options: ShortcutOptions) -> ExtensionResult<()> { Err("unused capability".into()) }
    fn register_flag(&self, _name: &str, _options: FlagOptions) -> ExtensionResult<()> { Err("unused capability".into()) }
    fn get_flag(&self, _name: &str) -> ExtensionResult<Option<FlagValue>> { Err("unused capability".into()) }
    fn register_tool(&self, _tool: ToolDefinition) -> ExtensionResult<()> { Err("unused capability".into()) }
    fn register_message_renderer(&self, _custom_type: &str, _renderer: crate::MessageRenderer) -> ExtensionResult<()> { Err("unused capability".into()) }
    fn append_entry(&self, _custom_type: &str, _data: Option<Value>) -> ExtensionResult<()> { Err("unused capability".into()) }
    fn send_message(&self, _message: CustomMessageInput, _options: Option<SendMessageOptions>) -> ExtensionResult<()> { Err("unused capability".into()) }
    fn send_user_message(&self, _content: UserContent, _options: Option<SendUserMessageOptions>) -> ExtensionResult<()> { Err("unused capability".into()) }
    fn set_session_name(&self, _name: &str) -> ExtensionResult<()> { Err("unused capability".into()) }
    fn get_session_name(&self) -> ExtensionResult<Option<String>> { Err("unused capability".into()) }
    fn set_label(&self, _entry_id: &str, _label: Option<&str>) -> ExtensionResult<()> { Err("unused capability".into()) }
    fn get_active_tools(&self) -> ExtensionResult<Vec<String>> { Err("unused capability".into()) }
    fn get_all_tools(&self) -> ExtensionResult<Vec<ToolInfo>> { Err("unused capability".into()) }
    fn set_active_tools(&self, _tool_names: &[String]) -> ExtensionResult<()> { Err("unused capability".into()) }
    fn get_commands(&self) -> ExtensionResult<Vec<SlashCommandInfo>> { Err("unused capability".into()) }
    fn get_thinking_level(&self) -> ExtensionResult<ThinkingLevel> { Err("unused capability".into()) }
    fn set_thinking_level(&self, _level: ThinkingLevel) -> ExtensionResult<()> { Err("unused capability".into()) }
    fn events(&self) -> EventBus { EventBus::new(self.clone()) }
    fn set_model(&self, _model: Model) -> ExtensionFuture<'_, bool> { Box::pin(async { Err("unused capability".into()) }) }
}
impl EventBusPort for Host {
    fn on(&self, channel: &str, listener: EventListener) -> ExtensionResult<Subscription> {
        assert_eq!(channel, autonomy::TRIGGER_CHANNEL);
        self.listener.replace(Some(listener));
        Ok(Subscription::new(Rc::new(Remover)))
    }
    fn emit<'a>(&'a self, channel: &'a str, data: Value) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            assert_eq!(channel, autonomy::TRIGGER_CHANNEL);
            self.payloads.borrow_mut().push(data.clone());
            let listener = self.listener.borrow().clone().expect("registered listener");
            let tail = listener(data, CallbackEmitter::new(self))?;
            assert!(tail.is_some(), "listener must hand off an owned lifecycle tail");
            Ok(())
        })
    }
}
impl CallbackEmitterPort for Host {
    fn emit(&self, _channel: &str, _data: Value) -> ExtensionResult<()> { Err("unused callback emission".into()) }
}
/// Subscription retained by the recording bus.
struct Remover;
impl SubscriptionPort for Remover {
    fn unsubscribe(&self) -> ExtensionResult<()> { Ok(()) }
}
/// Command context deliberately rejecting unused capabilities.
struct CommandContext;
impl ContextPort for CommandContext {
    fn cwd(&self) -> ExtensionResult<String> { Err("unused capability".into()) }
    fn compact(&self, _options: Option<CompactOptions>) -> ExtensionResult<()> { Err("unused capability".into()) }
    fn ui(&self) -> ExtensionResult<ExtensionUIContext> { Err("unused capability".into()) }
    fn has_ui(&self) -> ExtensionResult<bool> { Err("unused capability".into()) }
    fn model_registry(&self) -> ExtensionResult<ModelRegistry> { Err("unused capability".into()) }
    fn model(&self) -> ExtensionResult<Option<Model>> { Err("unused capability".into()) }
    fn is_idle(&self) -> ExtensionResult<bool> { Err("unused capability".into()) }
    fn signal(&self) -> ExtensionResult<Option<AbortSignal>> { Err("unused capability".into()) }
    fn abort(&self) -> ExtensionResult<()> { Err("unused capability".into()) }
    fn has_pending_messages(&self) -> ExtensionResult<bool> { Err("unused capability".into()) }
    fn shutdown(&self) -> ExtensionResult<()> { Err("unused capability".into()) }
    fn get_context_usage(&self) -> ExtensionResult<Option<ContextUsage>> { Err("unused capability".into()) }
    fn get_system_prompt(&self) -> ExtensionResult<String> { Err("unused capability".into()) }
    fn session_manager(&self) -> ExtensionResult<ReadonlySessionManager> { Err("unused capability".into()) }
}
impl CommandContextPort for CommandContext {
    fn fork<'a>(&'a self, _entry_id: &'a str, _options: Option<crate::ForkOptions>,) -> ExtensionFuture<'a, SessionChangeResult> { Box::pin(async { Err("unused command capability".into()) }) }
    fn navigate_tree<'a>(&'a self, _target_id: &'a str, _options: Option<crate::NavigateTreeOptions>,) -> ExtensionFuture<'a, SessionChangeResult> { Box::pin(async { Err("unused command capability".into()) }) }
    fn switch_session<'a>(&'a self, _path: &'a str, _options: Option<crate::SwitchSessionOptions>,) -> ExtensionFuture<'a, SessionChangeResult> { Box::pin(async { Err("unused command capability".into()) }) }
    fn reload(&self) -> ExtensionFuture<'_, ()> { Box::pin(async { Err("unused command capability".into()) }) }
    fn wait_for_idle(&self) -> ExtensionFuture<'_, ()> { Box::pin(async { Err("unused command capability".into()) }) }
    fn new_session(&self, _options: Option<NewSessionCommandOptions>,) -> ExtensionFuture<'_, SessionChangeResult> { Box::pin(async { Err("unused command capability".into()) }) }
}
/// Poll a future whose test ports never suspend.
fn ready<T>(future: impl Future<Output = T>) -> T {
    let mut future = Box::pin(future);
    match future.as_mut().poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("test port suspended"),
    }
}
/// Assessment registration dispatches one trigger and rejects malformed listener input.
#[test]
fn registered_assess_dispatches_to_the_trigger_listener() {
    let host = Rc::new(Host::default());
    register_autonomy(&ExtensionAPI::new(Rc::new(host.clone()))).expect("register");
    let names: Vec<_> = host.commands.borrow().keys().cloned().collect();
    assert_eq!(names, ["assess"]);
    let home = std::env::temp_dir().join(format!("security-boundary-{}", std::process::id()));
    crate::evidence::with_home(&home, || {
        let handler = host.commands.borrow()["assess"].handler.clone();
        ready(handler("  example.test  ".into(), ExtensionCommandContext::new(Rc::new(CommandContext)))).expect("command");
        let payloads = host.payloads.borrow();
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0]["target"], "example.test");
        let run_id = payloads[0]["run_id"].as_str().expect("run id");
        assert_eq!(crate::store::Store.get_run(run_id).expect("index").expect("run").target, "example.test");
        let listener = host.listener.borrow().clone().expect("listener");
        let tail = listener(json!({"run_id": run_id}), CallbackEmitter::new(host.as_ref())).expect("prefix").expect("tail");
        assert_eq!(ready(tail).expect_err("malformed payload"), "trigger payload needs run_id and target");
    });
    let _ = std::fs::remove_dir_all(home);
}
