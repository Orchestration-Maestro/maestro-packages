//! The locked Maestro source the tests read documentation and assets from.
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The checkout of the locked Maestro source: two levels above its guest crate's directory.
pub fn checkout() -> &'static Path {
    static CHECKOUT: OnceLock<PathBuf> = OnceLock::new();
    CHECKOUT.get_or_init(|| {
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let output = std::process::Command::new(cargo)
            .args(["metadata", "--format-version", "1", "--locked", "--offline"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let guest = metadata["packages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|package| package["name"] == "maestro-extensions-wasm")
            .unwrap();
        Path::new(guest["manifest_path"].as_str().unwrap())
            .ancestors()
            .nth(3)
            .unwrap()
            .to_path_buf()
    })
}
