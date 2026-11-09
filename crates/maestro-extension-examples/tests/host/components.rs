//! Built components loaded through the domain loader on a local task set.
#![cfg(test)]
use maestro_extensions::{EventBusController, Local, ModuleLoader, create_event_bus_for};
use std::rc::Rc;

/// Runs a test body on the local task set the artifact factory requires.
pub fn local<T>(test: impl Future<Output = T>) -> T {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, test)
}

/// Loads every module path through the artifact factory, as an application's module loader does.
pub fn loader() -> ModuleLoader<Local> {
    Rc::new(|path: &str| {
        Box::pin(std::future::ready(Ok(Some(super::support::artifact(
            path.to_owned(),
        )))))
    })
}

/// A bus every loaded component receives; a tail or listener error fails the test.
pub struct Bus {
    /// The bus every loaded component receives.
    pub controller: EventBusController<Local>,
}

impl Bus {
    /// A bus whose tails and listener errors panic.
    pub fn new() -> Self {
        Self {
            controller: create_event_bus_for::<Local>(
                Rc::new(|_| panic!("no tails")),
                Rc::new(|_| panic!("no listener errors")),
            ),
        }
    }
}
