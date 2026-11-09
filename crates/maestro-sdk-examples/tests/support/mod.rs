//! Runtimes and task sets the demo checks share.

pub mod session_harness;

/// Build one current-thread runtime with its I/O and time drivers.
pub fn io_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}
