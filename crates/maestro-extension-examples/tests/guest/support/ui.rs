//! A terminal UI that records the notifications an extension shows and can refuse them.
use maestro_extensions_wasmtime::test_support::maestro_extensions::{
    EventError, ExtensionRunner, ExtensionUIContext, Local,
    types::{ExtensionUIContextPort, NotificationType},
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// The notifications delivered to the UI, in order, and the host error the next one fails with.
#[derive(Default)]
pub struct Notifications {
    /// Each notification as the message and the severity in the host's own spelling.
    seen: RefCell<Vec<Value>>,
    /// The failure the next notification returns, once.
    rejection: RefCell<Option<String>>,
}

impl Notifications {
    /// Installs a recording UI on `runner` and returns it.
    pub fn install(runner: &ExtensionRunner<Local>) -> Rc<Self> {
        let ui = Rc::new(Self::default());
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
        ui
    }

    /// Makes the next notification fail with `message`.
    pub fn reject_next(&self, message: Option<&str>) {
        *self.rejection.borrow_mut() = message.map(str::to_owned);
    }

    /// Takes everything recorded since the last call.
    pub fn take(&self) -> Vec<Value> {
        std::mem::take(&mut self.seen.borrow_mut())
    }
}

impl ExtensionUIContextPort for Notifications {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::default()
    }

    fn notify(&self, message: String, kind: Option<NotificationType>) -> Result<(), EventError> {
        let kind = kind.map(|kind| format!("{kind:?}"));
        self.seen
            .borrow_mut()
            .push(json!({"message": message, "kind": kind}));
        match self.rejection.borrow_mut().take() {
            Some(rejection) => Err(rejection.into()),
            None => Ok(()),
        }
    }
}
