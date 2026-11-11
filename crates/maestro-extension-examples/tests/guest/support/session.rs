//! The application operations and session state a runner is bound to.
use maestro_extensions::{
    ExtensionContextActions, ExtensionExecution, ExtensionRunner, Local, Presence, ProviderConfig,
    ProviderModelConfig, SessionManager, SharedHandle,
    types::{ExtensionActions, ProviderActions},
};
use maestro_extensions_wasmtime::test_support::{Harness, unconfigured_context_actions};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

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

/// Binds `runner` to operations that start from the ones nothing serves and are then adjusted
/// by `serve`.
pub fn bind(
    runner: &ExtensionRunner<Local>,
    serve: impl FnOnce(&mut ExtensionActions<Local>, &mut ExtensionContextActions),
) {
    let mut bindings = Bindings::new();
    serve(&mut bindings.actions, &mut bindings.context);
    bindings.apply(runner);
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

/// Fails with the message in `slot`, if it holds one.
pub fn fail_with(slot: &RefCell<Option<String>>) -> Result<(), maestro_extensions::EventError> {
    match slot.borrow().clone() {
        Some(message) => Err(message.into()),
        None => Ok(()),
    }
}
