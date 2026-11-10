//! The failures a runner reports for callbacks whose errors do not propagate.
use maestro_extensions::{ExtensionRunner, Local};
use std::{cell::RefCell, rc::Rc};

/// The messages of the failures a runner's callbacks reported, oldest first.
#[derive(Clone, Default)]
pub struct Failures(Rc<RefCell<Vec<String>>>);

impl Failures {
    /// Starts collecting the failures `runner` reports.
    pub fn collect(runner: &ExtensionRunner<Local>) -> Self {
        let failures = Self::default();
        let sink = failures.0.clone();
        let remove = runner.on_error(Rc::new(move |error| {
            sink.borrow_mut().push(error.error.clone());
            Ok(())
        }));
        drop(remove);
        failures
    }

    /// Takes the messages collected since the last call.
    pub fn take(&self) -> Vec<String> {
        std::mem::take(&mut self.0.borrow_mut())
    }

    /// The messages collected so far.
    pub fn seen(&self) -> Vec<String> {
        self.0.borrow().clone()
    }
}
