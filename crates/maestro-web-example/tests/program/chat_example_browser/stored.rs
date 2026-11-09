//! Browser storage whose committed writes are signalled, so a test waits for the save it needs.
use crate::browser_fixture_support::{Patch, get, global, invoke, object, signal};
use maestro_app::{AppStorage, SessionMetadata};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsValue, closure::Closure};

/// The listener that counts one committed write and wakes the waits for it.
fn committed(counted: Rc<Cell<u32>>, woken: Rc<RefCell<Vec<js_sys::Function>>>) -> JsValue {
    Closure::once_into_js(move || {
        counted.set(counted.get() + 1);
        for resolve in woken.take() {
            resolve.call0(&JsValue::NULL).unwrap();
        }
    })
}

/// Storage for the example's model key, with a signal for every committed write.
pub(super) struct Stored {
    /// The storage the page under test uses.
    storage: Rc<AppStorage>,
    /// How many write transactions have committed.
    commits: Rc<Cell<u32>>,
    /// Resolvers of the waits for the next commit.
    waiting: Rc<RefCell<Vec<js_sys::Function>>>,
    /// The replaced transaction start, restored on drop.
    _transaction: Patch,
}

impl Stored {
    /// Storage named `name` holding a key for the example's model, watched from now on.
    pub(super) async fn keyed(name: &str) -> Self {
        let storage = super::app::storage(name);
        let commits = Rc::new(Cell::new(0));
        let waiting: Rc<RefCell<Vec<js_sys::Function>>> = Rc::default();
        let (counted, woken) = (Rc::clone(&commits), Rc::clone(&waiting));
        let prototype = get(&global("IDBDatabase"), "prototype");
        let transaction = Patch::method(
            &prototype,
            "transaction",
            move |original, database, args| {
                let started = original.apply(&database, &args)?;
                if args.get(1).as_string().as_deref() == Some("readwrite") {
                    let on_complete = committed(Rc::clone(&counted), Rc::clone(&woken));
                    let once = object(&[("once", &JsValue::TRUE)]);
                    invoke(
                        &started,
                        "addEventListener",
                        &[&"complete".into(), &on_complete, &once],
                    )?;
                }
                Ok(started)
            },
        );
        storage
            .provider_keys
            .set("anthropic", "sk-test")
            .await
            .unwrap();
        Self {
            storage,
            commits,
            waiting,
            _transaction: transaction,
        }
    }

    /// The storage, shared with the page under test.
    pub(super) fn shared(&self) -> Rc<AppStorage> {
        Rc::clone(&self.storage)
    }

    /// The stored summary of `id` once a committed save satisfies `ready`.
    pub(super) async fn stored(
        &self,
        id: &str,
        ready: impl Fn(&SessionMetadata) -> bool,
    ) -> SessionMetadata {
        loop {
            let seen = self.commits.get();
            if let Some(metadata) = self.storage.sessions.get_metadata(id).await.unwrap()
                && ready(&metadata)
            {
                return metadata;
            }
            if self.commits.get() == seen {
                let (committed, resolve) = signal();
                self.waiting.borrow_mut().push(resolve);
                crate::contributions_support::settle(committed.into())
                    .await
                    .unwrap();
            }
        }
    }
}
