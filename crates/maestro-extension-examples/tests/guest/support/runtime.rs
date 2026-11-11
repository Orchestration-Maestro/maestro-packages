//! The runtimes that loaded components run on.
use maestro_extensions_wasmtime::test_support::local as local_tasks;

/// Runs a test body on the local task set that loaded components need.
pub fn local<T>(test: impl Future<Output = T>) -> T {
    local_tasks(test).unwrap()
}

/// Runs a test body on a local task set whose runtime has the timer and process drivers that
/// components waiting on the clock or on child processes need.
pub fn driven<T>(test: impl Future<Output = T>) -> T {
    maestro_test_process::executor::block_on(test).unwrap()
}

/// Runs a test body on a current-thread runtime whose clock is paused, inside the local task
/// set that loaded components need. The clock moves only while every task waits, so a native
/// wait of any length ends at once.
pub fn timed<T>(test: impl Future<Output = T>) -> T {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, test)
}

/// Runs a test body like [`timed`], but the clock moves only when the test advances it.
///
/// A task that always has work keeps the runtime from parking, and a paused clock moves
/// forward on its own only while the runtime is parked.
pub fn stepped<T>(test: impl Future<Output = T>) -> T {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let busy = tokio::task::spawn_local(async {
            loop {
                tokio::task::yield_now().await;
            }
        });
        let outcome = test.await;
        busy.abort();
        outcome
    })
}
