//! The footer data a test controls.
use maestro_extensions::footer::{ExtensionStatuses, ReadonlyFooterDataProvider};
use std::{cell::RefCell, rc::Rc};

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
