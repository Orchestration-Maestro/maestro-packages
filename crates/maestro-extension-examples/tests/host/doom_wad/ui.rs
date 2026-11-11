//! A custom view that runs a scenario against the real overlay before closing it through input.
use maestro_extensions::{
    EventError,
    presentation::ExtensionKeybindings,
    types::{
        CustomComponentFactory, CustomFactoryOutcome, CustomUIOptions, ExtensionComponent,
        ExtensionUIContextPort, ExtensionUIFuture, NotificationType,
    },
};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
};

/// What a test does with a view the overlay opened, before the view is closed.
pub type Scenario = Box<dyn FnOnce(&ExtensionComponent) -> Result<(), EventError>>;

/// Visible notices and the views the overlay opens.
pub struct Ui {
    /// Real terminal supplied to custom factories.
    pub tui: maestro_tui::TUI,
    /// All user-facing command notices.
    pub notices: RefCell<Vec<String>>,
    /// One scenario per view the overlay is expected to open.
    pub scenarios: RefCell<VecDeque<Scenario>>,
    /// Completes when the overlay reports that the game stopped.
    pub stopped: Rc<tokio::sync::Notify>,
    /// Whether the next view stays open until the game stops instead of closing at once.
    pub until_stopped: Cell<bool>,
}
impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::default()
    }
    fn notify(&self, message: String, _: Option<NotificationType>) -> Result<(), EventError> {
        if message.starts_with("DOOM stopped:") {
            self.stopped.notify_one();
        }
        self.notices.borrow_mut().push(message);
        Ok(())
    }
    fn custom_component(
        &self,
        factory: CustomComponentFactory,
        _: Option<CustomUIOptions>,
    ) -> ExtensionUIFuture<()> {
        let (send, receive) = tokio::sync::oneshot::channel();
        let send = RefCell::new(Some(send));
        let close = Rc::new(move || {
            Ok(send
                .borrow_mut()
                .take()
                .is_some_and(|send| send.send(()).is_ok()))
        });
        let keys = ExtensionKeybindings {
            manager: maestro_tui::KeybindingsManager::new(Vec::new(), Vec::new()),
            reload: None,
        };
        let building = factory.call((self.tui.clone(), self.theme(), keys, close));
        let scenario = self.scenarios.borrow_mut().pop_front();
        let until_stopped = self
            .until_stopped
            .replace(false)
            .then(|| self.stopped.clone());
        Box::pin(async move {
            let built = match building? {
                CustomFactoryOutcome::Ready(built) => built,
                CustomFactoryOutcome::Async(building) => building.await?,
            };
            if let Some(scenario) = scenario {
                scenario(&built)?;
            }
            if let Some(stopped) = until_stopped {
                stopped.notified().await;
            }
            built
                .component
                .input_handler()
                .unwrap()
                .try_handle_input("q")
                .map_err(|e| EventError::from(e.to_string()))?;
            receive.await.map_err(|e| EventError::from(e.to_string()))?;
            if let Some(dispose) = built.dispose {
                dispose.call(())?;
            }
            Ok(())
        })
    }
}
