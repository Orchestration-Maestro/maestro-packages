//! Disposable directories for native peers.

/// A directory removed when dropped, on a best-effort basis.
pub struct Scratch(pub maestro_test_directory::Scratch);

impl Scratch {
    /// Creates a fresh directory.
    pub fn new() -> Self {
        Self(maestro_test_directory::Scratch::new().expect("scratch directory is new"))
    }

    /// The path of `name` inside the directory.
    pub fn join(&self, name: &str) -> String {
        self.0.path().join(name).to_string_lossy().into_owned()
    }
}
