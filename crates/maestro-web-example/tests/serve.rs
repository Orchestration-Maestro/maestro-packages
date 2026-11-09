//! The recipe that serves the bound chat example, run through `just` with a stand-in server.
#![cfg(unix)]

use maestro_test_directory::Scratch;
use std::{path::Path, process::Command};

/// Run `just browser-chat-serve` with `arguments`, the server replaced by a command that prints
/// each argument it receives on its own line, and return those lines.
fn served(target: &Path, arguments: &[&str]) -> Vec<String> {
    let output = Command::new("just")
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .env("CARGO_TARGET_DIR", target)
        .args(["--set", "serve", "printf '%s\\n'", "browser-chat-serve"])
        .args(arguments)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// Each extra argument reaches the server as its own argument, and the served directory sits in
/// the artifact directory Cargo was told to use.
#[test]
fn serving_forwards_each_argument_from_cargos_target_directory() {
    let target = Scratch::new().unwrap();
    assert_eq!(
        served(target.path(), &["--port", "9000"]),
        [
            target.path().join("maestro-browser/chat").to_str().unwrap(),
            "--interfaces",
            "127.0.0.1",
            "--index",
            "index.html",
            "--port",
            "9000",
        ]
    );
}
