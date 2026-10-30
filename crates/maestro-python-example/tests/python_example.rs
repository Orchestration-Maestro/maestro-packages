//! Builds the Python example with the patched componentize-py and runs it in a Maestro executable.
//!
//! The test is ignored unless asked for. It reads the extension world from `target/wit`, the
//! executable from `target/maestro` and the compiler from `PATH`; the example's README says
//! how to place them.
#![cfg(test)]

#[path = "../../maestro-typescript-example/tests/host/mod.rs"]
mod host;

use std::path::Path;
use std::process::Command;

use serde_json::json;

use host::{Run, Scratch, Turn, describe, last_text, root};

/// What the model answers once it has the tool's result.
const FINAL_REPLY: &str = "Greeting delivered.";

/// Compiles the example's `hello` module against the unchanged world.
fn build(component: &Path) {
    let output = Command::new("componentize-py")
        .arg("-d")
        .arg(root().join("target/wit"))
        .args(["-w", "extension", "componentize", "-p"])
        .arg(root().join("examples/python"))
        .arg("hello")
        .arg("-o")
        .arg(component)
        .output()
        .expect("componentize-py must be on PATH; the example's README says which one");
    assert!(
        output.status.success(),
        "componentize-py failed: {}",
        describe(&output)
    );
}

/// The installed component upper-cases the prompt, is offered to the model as a tool and
/// answers the model's call, all through the host.
#[test]
#[ignore = "needs a Maestro executable in target/maestro and the world in target/wit"]
fn the_host_runs_the_component() {
    let scratch = Scratch::new("python");
    let component = scratch.0.join("hello.wasm");
    build(&component);
    let call = Turn::Call {
        tool: "hello",
        arguments: "{\"name\":\"Ada\"}",
    };

    let Run { requests, output } = host::run(
        &scratch.0,
        &[&component],
        "greet Ada",
        &[call, Turn::Say(FINAL_REPLY)],
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{}", describe(&output));
    assert!(
        !describe(&output).contains("component fault"),
        "{}",
        describe(&output)
    );
    assert_eq!(
        requests.len(),
        2,
        "the model is called before and after the tool"
    );
    assert_eq!(
        last_text(&requests[0]),
        &json!("GREET ADA"),
        "the input hook ran"
    );
    assert_eq!(
        requests[0]["tools"][0]["function"]["name"], "hello",
        "the tool is offered"
    );
    let answer = requests[1]["messages"].as_array().unwrap().last().unwrap();
    assert_eq!(
        answer,
        &json!({"role": "tool", "content": "Hello, Ada!", "tool_call_id": "call_1"})
    );
    assert!(stdout.contains(FINAL_REPLY), "{}", describe(&output));
}
