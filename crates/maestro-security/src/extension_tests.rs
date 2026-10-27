//! Behavioral checks at the registered command and listener boundary.
use super::factory;
use crate::autonomy;
use maestro_extensions_wasm::{
    AbortSignal, CallbackEmitter, CallbackEmitterPort, CommandContextPort, CommandOptions,
    CompactOptions, ContextPort, ContextUsage, CustomMessageInput, EventBus, EventBusPort,
    EventListener, ExtensionAPI, ExtensionCommandContext, ExtensionFuture, ExtensionHandler,
    ExtensionHost, ExtensionResult, ExtensionUIContext, FlagOptions, FlagValue, ForkOptions,
    MessageRenderer, Model, ModelRegistry, NavigateTreeOptions, NewSessionCommandOptions,
    ReadonlySessionManager, SendMessageOptions, SendUserMessageOptions, SessionChangeResult,
    ShortcutOptions, SlashCommandInfo, Subscription, SubscriptionPort, SwitchSessionOptions,
    ThinkingLevel, ToolDefinition, ToolInfo, UserContent,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    future::Future,
    rc::Rc,
    task::{Context, Poll, Waker},
};

/// Host capturing registrations and emitted payloads without running network work.
#[derive(Default)]
struct Host {
    /// Registered handlers, invoked by the tests.
    commands: RefCell<BTreeMap<String, CommandOptions>>,
    /// Registered tools invoked through their exported callbacks.
    tools: RefCell<BTreeMap<String, ToolDefinition>>,
    /// Host refusal used to check registration error propagation.
    reject_tools: RefCell<bool>,
    /// Simulated trigger delivery refusal.
    reject_emits: RefCell<bool>,
    /// Registered trigger listener.
    listener: RefCell<Option<EventListener>>,
    /// Payloads delivered through the registered listener boundary.
    payloads: RefCell<Vec<Value>>,
}
/// API adapter sharing the recording bus owner.
struct ApiHost(Rc<Host>);
impl ExtensionHost for ApiHost {
    fn on(&self, _event: &str, _handler: ExtensionHandler) -> ExtensionResult<()> {
        Err("unused capability".into())
    }
    fn register_command(&self, name: &str, options: CommandOptions) -> ExtensionResult<()> {
        self.0.commands.borrow_mut().insert(name.into(), options);
        Ok(())
    }
    fn register_shortcut(&self, _shortcut: &str, _options: ShortcutOptions) -> ExtensionResult<()> {
        Err("unused capability".into())
    }
    fn register_flag(&self, _name: &str, _options: FlagOptions) -> ExtensionResult<()> {
        Err("unused capability".into())
    }
    fn get_flag(&self, _name: &str) -> ExtensionResult<Option<FlagValue>> {
        Err("unused capability".into())
    }
    fn register_tool(&self, tool: ToolDefinition) -> ExtensionResult<()> {
        if *self.0.reject_tools.borrow() {
            return Err("registration refused".into());
        }
        self.0
            .tools
            .borrow_mut()
            .insert(tool.metadata.name.clone(), tool);
        Ok(())
    }
    fn register_message_renderer(
        &self,
        _custom_type: &str,
        _renderer: MessageRenderer,
    ) -> ExtensionResult<()> {
        Err("unused capability".into())
    }
    fn append_entry(&self, _custom_type: &str, _data: Option<Value>) -> ExtensionResult<()> {
        Err("unused capability".into())
    }
    fn send_message(
        &self,
        _message: CustomMessageInput,
        _options: Option<SendMessageOptions>,
    ) -> ExtensionResult<()> {
        Err("unused capability".into())
    }
    fn send_user_message(
        &self,
        _content: UserContent,
        _options: Option<SendUserMessageOptions>,
    ) -> ExtensionResult<()> {
        Err("unused capability".into())
    }
    fn set_session_name(&self, _name: &str) -> ExtensionResult<()> {
        Err("unused capability".into())
    }
    fn get_session_name(&self) -> ExtensionResult<Option<String>> {
        Err("unused capability".into())
    }
    fn set_label(&self, _entry_id: &str, _label: Option<&str>) -> ExtensionResult<()> {
        Err("unused capability".into())
    }
    fn get_active_tools(&self) -> ExtensionResult<Vec<String>> {
        Err("unused capability".into())
    }
    fn get_all_tools(&self) -> ExtensionResult<Vec<ToolInfo>> {
        Err("unused capability".into())
    }
    fn set_active_tools(&self, _tool_names: &[String]) -> ExtensionResult<()> {
        Err("unused capability".into())
    }
    fn get_commands(&self) -> ExtensionResult<Vec<SlashCommandInfo>> {
        Err("unused capability".into())
    }
    fn get_thinking_level(&self) -> ExtensionResult<ThinkingLevel> {
        Err("unused capability".into())
    }
    fn set_thinking_level(&self, _level: ThinkingLevel) -> ExtensionResult<()> {
        Err("unused capability".into())
    }
    fn events(&self) -> EventBus {
        EventBus::new(self.0.clone())
    }
    fn set_model(&self, _model: Model) -> ExtensionFuture<'_, bool> {
        Box::pin(async { Err("unused capability".into()) })
    }
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
            if *self.reject_emits.borrow() {
                return Err("delivery refused".into());
            }
            self.payloads.borrow_mut().push(data.clone());
            let listener = self.listener.borrow().clone().expect("registered listener");
            let tail = listener(data, CallbackEmitter::new(self))?;
            assert!(
                tail.is_some(),
                "listener must hand off an owned lifecycle tail"
            );
            Ok(())
        })
    }
}
impl CallbackEmitterPort for Host {
    fn emit(&self, _channel: &str, _data: Value) -> ExtensionResult<()> {
        Err("unused callback emission".into())
    }
}
/// Subscription retained by the recording bus.
struct Remover;
impl SubscriptionPort for Remover {
    fn unsubscribe(&self) -> ExtensionResult<()> {
        Ok(())
    }
}
/// Command context deliberately rejecting unused capabilities.
struct CommandContext;
impl ContextPort for CommandContext {
    fn cwd(&self) -> ExtensionResult<String> {
        Err("unused capability".into())
    }
    fn compact(&self, _options: Option<CompactOptions>) -> ExtensionResult<()> {
        Err("unused capability".into())
    }
    fn ui(&self) -> ExtensionResult<ExtensionUIContext> {
        Err("unused capability".into())
    }
    fn has_ui(&self) -> ExtensionResult<bool> {
        Err("unused capability".into())
    }
    fn model_registry(&self) -> ExtensionResult<ModelRegistry> {
        Err("unused capability".into())
    }
    fn model(&self) -> ExtensionResult<Option<Model>> {
        Err("unused capability".into())
    }
    fn is_idle(&self) -> ExtensionResult<bool> {
        Err("unused capability".into())
    }
    fn signal(&self) -> ExtensionResult<Option<AbortSignal>> {
        Err("unused capability".into())
    }
    fn abort(&self) -> ExtensionResult<()> {
        Err("unused capability".into())
    }
    fn has_pending_messages(&self) -> ExtensionResult<bool> {
        Err("unused capability".into())
    }
    fn shutdown(&self) -> ExtensionResult<()> {
        Err("unused capability".into())
    }
    fn get_context_usage(&self) -> ExtensionResult<Option<ContextUsage>> {
        Err("unused capability".into())
    }
    fn get_system_prompt(&self) -> ExtensionResult<String> {
        Err("unused capability".into())
    }
    fn session_manager(&self) -> ExtensionResult<ReadonlySessionManager> {
        Err("unused capability".into())
    }
}
impl CommandContextPort for CommandContext {
    fn fork<'a>(
        &'a self,
        _entry_id: &'a str,
        _options: Option<ForkOptions>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        Box::pin(async { Err("unused command capability".into()) })
    }
    fn navigate_tree<'a>(
        &'a self,
        _target_id: &'a str,
        _options: Option<NavigateTreeOptions>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        Box::pin(async { Err("unused command capability".into()) })
    }
    fn switch_session<'a>(
        &'a self,
        _path: &'a str,
        _options: Option<SwitchSessionOptions>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        Box::pin(async { Err("unused command capability".into()) })
    }
    fn reload(&self) -> ExtensionFuture<'_, ()> {
        Box::pin(async { Err("unused command capability".into()) })
    }
    fn wait_for_idle(&self) -> ExtensionFuture<'_, ()> {
        Box::pin(async { Err("unused command capability".into()) })
    }
    fn new_session(
        &self,
        _options: Option<NewSessionCommandOptions>,
    ) -> ExtensionFuture<'_, SessionChangeResult> {
        Box::pin(async { Err("unused command capability".into()) })
    }
}
/// Poll a future whose test ports never suspend.
fn ready<T>(future: impl Future<Output = T>) -> T {
    let mut future = Box::pin(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("test port suspended"),
    }
}
/// Assessment registration dispatches one trigger and rejects malformed listener input.
#[test]
fn registered_assess_dispatches_to_the_trigger_listener() {
    let host = Rc::new(Host::default());
    ready(factory(ExtensionAPI::new(Rc::new(ApiHost(host.clone()))))).expect("register");
    let names: Vec<_> = host.commands.borrow().keys().cloned().collect();
    assert_eq!(names, ["assess"]);
    let home = std::env::temp_dir().join(format!("security-boundary-{}", std::process::id()));
    crate::evidence::with_home(&home, || {
        let handler = host.commands.borrow()["assess"].handler.clone();
        ready(handler(
            "  example.test  ".into(),
            ExtensionCommandContext::new(Rc::new(CommandContext)),
        ))
        .expect("command");
        let payloads = host.payloads.borrow();
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0]["target"], "example.test");
        let run_id = payloads[0]["run_id"].as_str().expect("run id");
        assert_eq!(
            crate::store::Store
                .get_run(run_id)
                .expect("index")
                .expect("run")
                .target,
            "example.test"
        );
        let listener = host.listener.borrow().clone().expect("listener");
        let tail = listener(
            json!({"run_id": run_id}),
            CallbackEmitter::new(host.as_ref()),
        )
        .expect("prefix")
        .expect("tail");
        assert_eq!(
            ready(tail).expect_err("malformed payload"),
            "trigger payload needs run_id and target"
        );
    });
    let _ = std::fs::remove_dir_all(home);
}

/// Baseline: every registered tool validates its own required input.
#[test]
fn baseline_registered_tools_reject_missing_arguments() {
    let host = Rc::new(Host::default());
    ready(factory(ExtensionAPI::new(Rc::new(ApiHost(host.clone()))))).expect("register");
    for (name, field) in [
        ("security_assess", "target"),
        ("security_evidence", "run_id"),
        ("security_workspace", "action"),
        ("security_route", "task"),
        ("security_report_page", "run_id"),
        ("security_evidence_page", "run_id"),
        ("security_validate", "run_id"),
        ("security_disclose", "action"),
    ] {
        let execute = host.tools.borrow()[name].execute.clone();
        let result = ready(execute(
            "call-1".into(),
            json!({}),
            None,
            None,
            maestro_extensions_wasm::ExtensionContext::new(Rc::new(CommandContext)),
        ));
        assert!(
            result.expect_err("missing argument").contains(field),
            "{name}"
        );
    }
}

/// Baseline: retained evidence is returned intact in details, not just a summary.
#[test]
fn baseline_registered_evidence_returns_the_retained_ledger() {
    let host = Rc::new(Host::default());
    ready(factory(ExtensionAPI::new(Rc::new(ApiHost(host.clone()))))).expect("register");
    let home =
        std::env::temp_dir().join(format!("security-extension-ledger-{}", std::process::id()));
    crate::evidence::with_home(&home, || {
        let mut ledger = crate::evidence::Ledger::new();
        ledger.record_exec("recon", "inspect", &"é".repeat(200), 0);
        ledger
            .save(
                "run-evidence",
                "example.test",
                "maestro.security",
                "fixture",
            )
            .expect("save");
        let execute = host.tools.borrow()["security_evidence"].execute.clone();
        let result = ready(execute(
            "call-1".into(),
            json!({"run_id":"run-evidence"}),
            None,
            None,
            maestro_extensions_wasm::ExtensionContext::new(Rc::new(CommandContext)),
        ))
        .expect("evidence");
        let maestro_extensions_wasm::Presence::Present(details) = result.details else {
            panic!("evidence must include full details");
        };
        let doc: Value = serde_json::from_str(&details).expect("ledger JSON");
        assert_eq!(doc["run_id"], "run-evidence");
        assert_eq!(doc["entries"][0]["output"], "é".repeat(200));
        assert_eq!(result.content.len(), 1);
    });
    std::fs::remove_dir_all(home).expect("remove ledger");
}

/// Baseline: a blank assess command cannot create a run or emit a trigger.
#[test]
fn baseline_blank_assess_does_not_start_a_run() {
    let host = Rc::new(Host::default());
    ready(factory(ExtensionAPI::new(Rc::new(ApiHost(host.clone()))))).expect("register");
    let home =
        std::env::temp_dir().join(format!("security-extension-blank-{}", std::process::id()));
    crate::evidence::with_home(&home, || {
        let handler = host.commands.borrow()["assess"].handler.clone();
        ready(handler(
            " \t\n ".into(),
            ExtensionCommandContext::new(Rc::new(CommandContext)),
        ))
        .expect("command");
        assert!(host.payloads.borrow().is_empty());
        assert!(crate::store::Store.list_runs(0).expect("index").is_empty());
    });
}

/// Baseline: registration errors reach the host without offering partial commands.
#[test]
fn baseline_factory_propagates_tool_registration_failure() {
    let host = Rc::new(Host::default());
    host.reject_tools.replace(true);
    let error = ready(factory(ExtensionAPI::new(Rc::new(ApiHost(host.clone())))))
        .expect_err("host rejection");
    assert_eq!(error, "registration refused");
    assert!(host.commands.borrow().is_empty());
    assert!(host.listener.borrow().is_none());
}

/// Baseline: failed trigger delivery settles the recorded run rather than leaving it live.
#[test]
fn baseline_failed_trigger_delivery_settles_the_run() {
    let host = Rc::new(Host::default());
    host.reject_emits.replace(true);
    ready(factory(ExtensionAPI::new(Rc::new(ApiHost(host.clone()))))).expect("register");
    let home =
        std::env::temp_dir().join(format!("security-extension-refused-{}", std::process::id()));
    crate::evidence::with_home(&home, || {
        let handler = host.commands.borrow()["assess"].handler.clone();
        ready(handler(
            "example.test".into(),
            ExtensionCommandContext::new(Rc::new(CommandContext)),
        ))
        .expect("command");
        let runs = crate::store::Store.list_runs(0).expect("index");
        assert_eq!(runs.len(), 1);
        assert!(matches!(runs[0].status, crate::store::Status::Failed));
        assert_eq!(runs[0].summary, "delivery refused");
        assert!(runs[0].finished_at.is_some());
    });
    std::fs::remove_dir_all(home).expect("remove index");
}

/// Baseline: registered read tools return usable data for retained runs.
#[test]
fn baseline_registered_read_tools_return_report_ledger_and_validation() {
    let host = Rc::new(Host::default());
    ready(factory(ExtensionAPI::new(Rc::new(ApiHost(host.clone()))))).expect("register");
    let home = std::env::temp_dir().join(format!("security-extension-read-{}", std::process::id()));
    crate::evidence::with_home(&home, || {
        let report: crate::Assessment = serde_json::from_value(json!({
            "schema":crate::SCHEMA, "run_id":"run-read", "target":"<target>",
            "agent":"maestro.security", "model":"fixture", "phases":[],
            "findings":[], "refuted":[], "threats":[], "unknown":[],
            "runtime_mechanics":[], "summary":"<summary>"
        }))
        .expect("report");
        crate::save_report("run-read", &report).expect("retain report");
        let mut ledger = crate::Ledger::new();
        ledger.record_note("recon", "<note>");
        ledger
            .save("run-read", "<target>", "maestro.security", "fixture")
            .expect("retain ledger");
        let invoke = |name: &str, arguments| {
            let execute = host.tools.borrow()[name].execute.clone();
            let result = ready(execute(
                "call-1".into(),
                arguments,
                None,
                None,
                maestro_extensions_wasm::ExtensionContext::new(Rc::new(CommandContext)),
            ))
            .expect("read tool");
            let maestro_extensions_wasm::UserBlock::Text(text) = &result.content[0] else {
                panic!("text result");
            };
            text.text.clone()
        };
        let arguments = json!({"run_id":"run-read"});
        let page = invoke("security_report_page", arguments.clone());
        assert!(page.contains("&lt;target&gt;"));
        assert!(!page.contains("<target>"));
        assert!(invoke("security_evidence_page", arguments.clone()).contains("&lt;note&gt;"));
        let validation: Value =
            serde_json::from_str(&invoke("security_validate", arguments)).expect("validation");
        assert_eq!(validation["violations"], json!([]));
        let sarif: Value = serde_json::from_str(&invoke(
            "security_disclose",
            json!({"action":"sarif","run_id":"run-read"}),
        ))
        .expect("SARIF");
        assert_eq!(sarif["version"], "2.1.0");
        let workspace: Value =
            serde_json::from_str(&invoke("security_workspace", json!({"action":"list"})))
                .expect("board");
        assert_eq!(workspace["action"], "list");
        assert!(invoke("security_route", json!({"task":"xyzzy"})).contains("no domain matched"));
    });
    std::fs::remove_dir_all(home).expect("remove retained state");
}
