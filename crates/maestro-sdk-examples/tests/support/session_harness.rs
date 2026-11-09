//! Task sets that drive a client on the current thread.

use std::rc::Rc;

/// Completes `body` on a fresh current-thread runtime inside a task set it receives.
pub fn on_task_set<F: std::future::Future>(
    body: impl FnOnce(Rc<tokio::task::LocalSet>) -> F,
) -> F::Output {
    let local = Rc::new(tokio::task::LocalSet::new());
    super::io_runtime().block_on(local.run_until(body(Rc::clone(&local))))
}
