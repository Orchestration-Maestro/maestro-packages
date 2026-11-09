//! The recipe that serves the bound chat example, run through `just` with a stand-in server.
#![cfg(unix)]

use maestro_test_directory::Scratch;
use std::{fs, path::Path, process::Command};

/// The argument recorder Cargo built for this test target.
const RECORDER: &str = env!("CARGO_BIN_EXE_argv-recorder");

/// Run `just browser-chat-serve` with `arguments`, the server replaced by `server`, and return the
/// lines it printed, one per argument it received.
fn served(server: &str, target: &Path, arguments: &[&str]) -> Vec<String> {
    let output = Command::new("just")
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .env("CARGO_TARGET_DIR", target)
        .args(["--set", "serve", server, "browser-chat-serve"])
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
        served(RECORDER, target.path(), &["--port", "9000"]),
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

/// A server path with a space in it still runs as one program, not as a command and an argument.
#[test]
fn serving_runs_a_server_whose_path_contains_spaces() {
    let target = Scratch::new().unwrap();
    let directory = target.path().join("server directory");
    fs::create_dir(&directory).unwrap();
    let server = directory.join("argv recorder");
    fs::copy(RECORDER, &server).unwrap();
    let arguments = ["--port", "9000"];
    assert_eq!(
        served(server.to_str().unwrap(), target.path(), &arguments),
        served(RECORDER, target.path(), &arguments)
    );
}
