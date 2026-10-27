//! Isolated state and assessment fixtures for capability tests.
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// Unique suffix for concurrent test directories.
static NEXT: AtomicU64 = AtomicU64::new(0);

/// Run with a fresh state directory, never the developer's retained evidence.
pub(crate) fn isolated(body: impl FnOnce(&Path)) {
    let home = std::env::temp_dir().join(format!(
        "maestro-security-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&home).expect("fresh state directory");
    crate::evidence::with_home(&home, || body(&home));
    std::fs::remove_dir_all(home).expect("remove test state");
}
