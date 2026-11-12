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
    future::Future,
    pin::Pin,
    rc::Rc,
};

use super::tics::Tics;

/// What a test does with a view the overlay opened, before the view is closed.
pub type Scenario = Box<dyn FnOnce(&ExtensionComponent) -> Result<(), EventError>>;

/// What a test does with a view the overlay opened while the game runs: the game advances
/// only while the test waits on the tics.
pub type Play = Box<
    dyn for<'a> FnOnce(
        &'a ExtensionComponent,
        &'a Tics,
    ) -> Pin<Box<dyn Future<Output = Result<(), EventError>> + 'a>>,
>;

/// Visible notices and the views the overlay opens.
pub struct Ui {
    /// Real terminal supplied to custom factories.
    pub tui: maestro_tui::TUI,
    /// All user-facing command notices.
    pub notices: RefCell<Vec<String>>,
    /// One scenario per view the overlay is expected to open.
    pub scenarios: RefCell<VecDeque<Scenario>>,
    /// The plays queued for the views the overlay opens, after their scenarios.
    pub plays: RefCell<VecDeque<Play>>,
    /// The frame requests of the terminal, one per game tic.
    pub tics: Tics,
    /// What the plays reported as failed, which shutdown checks.
    pub failures: Rc<RefCell<Vec<String>>>,
    /// Whether the next view stays open until the game closes it instead of the test pressing Q.
    pub until_closed: Cell<bool>,
}
impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::default()
    }
    fn notify(&self, message: String, _: Option<NotificationType>) -> Result<(), EventError> {
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
        self.tics.open();
        let closed = self.tics.clone();
        let close = Rc::new(move || {
            closed.close();
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
        let play = self.plays.borrow_mut().pop_front();
        let tics = self.tics.clone();
        let failures = self.failures.clone();
        let until_closed = self.until_closed.replace(false);
        Box::pin(async move {
            let built = match building? {
                CustomFactoryOutcome::Ready(built) => built,
                CustomFactoryOutcome::Async(building) => building.await?,
            };
            if let Some(scenario) = scenario {
                scenario(&built)?;
            }
            if let Some(play) = play {
                let outcome = play(&built, &tics).await;
                failures
                    .borrow_mut()
                    .extend(outcome.err().map(|error| error.to_string()));
            }
            if !until_closed {
                built
                    .component
                    .input_handler()
                    .unwrap()
                    .try_handle_input("q")
                    .map_err(|e| EventError::from(e.to_string()))?;
            }
            receive.await.map_err(|e| EventError::from(e.to_string()))?;
            if let Some(dispose) = built.dispose {
                dispose.call(())?;
            }
            Ok(())
        })
    }
}
