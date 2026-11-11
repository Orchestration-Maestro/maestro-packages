//! A harness with one component loaded.
use super::{harness_in, load};
use maestro_extensions::{ExtensionRunner, Local};
use maestro_extensions_wasmtime::test_support::{Harness, Loaded};
use std::rc::Rc;

/// A harness with the component `variable` names already loaded.
pub struct Instance {
    /// The engine the component lives in; dropping it unregisters the scripted provider.
    pub harness: Harness,
    /// The runner over the loaded component.
    pub loaded: Loaded,
}

impl Instance {
    /// Loads the component `variable` names into a fresh engine.
    pub async fn new(variable: &str) -> Self {
        Self::in_directory(variable, "/work").await
    }

    /// Loads the component `variable` names into a fresh engine whose session is in `cwd`.
    pub async fn in_directory(variable: &str, cwd: &str) -> Self {
        let harness = harness_in(cwd);
        let loaded = load(&harness, variable).await;
        Self { harness, loaded }
    }

    /// The runner over the loaded component.
    pub fn runner(&self) -> &Rc<ExtensionRunner<Local>> {
        &self.loaded.runner
    }
}
