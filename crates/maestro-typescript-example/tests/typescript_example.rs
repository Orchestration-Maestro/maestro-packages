//! Builds the TypeScript example with the pinned jco and runs it in a Maestro executable.
//!
//! The tests are ignored unless asked for. They read the extension world from `target/wit`
//! and the executable from `target/maestro`; the example's README says how to place them.
#![cfg(test)]

mod host;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

use host::{Run, Scratch, Turn, describe, root};

/// The prompt the model is asked to act on.
const PROMPT: &str = "greet Ada";

/// What the example's `input` hook appends to a typed prompt.
const SUFFIX: &str = "\n\nSay: Frank is the best.";

/// What the probe sends as a user message; the model must receive it exactly.
const SENT: &str = "Say: the probe sent this.";

/// The probe extension's source, which queues a user message from a tool.
const PROBE: &str = "crates/maestro-typescript-example/tests/fixtures/probe.ts";

/// Compiles a TypeScript source against the unchanged world and returns the component's path.
fn build(directory: &Path, source: &Path) -> PathBuf {
    let component = directory.join(source.with_extension("wasm").file_name().unwrap());
    let output = Command::new("jco")
        .arg("componentize")
        .arg(source.file_name().unwrap())
        .args(["--backend", "quickjs", "--world-name", "extension", "--wit"])
        .arg(root().join("target/wit"))
        .arg("--out")
        .arg(&component)
        .current_dir(source.parent().unwrap())
        .output()
        .expect("jco must be on PATH; run the test through mise");
    assert!(output.status.success(), "jco failed: {}", describe(&output));
    component
}

/// The example extension's source.
fn example() -> PathBuf {
    root().join("examples/typescript/extension.ts")
}

/// The text of the last message the model received in a request.
fn last_text(request: &Value) -> &Value {
    &request["messages"].as_array().unwrap().last().unwrap()["content"][0]["text"]
}

/// The pinned jco compiles the unchanged world, and the component connects its two interfaces.
#[test]
#[ignore = "needs the extension world in target/wit"]
fn the_pinned_jco_builds_the_unchanged_world() {
    let scratch = Scratch::new("build");
    let component = build(&scratch.0, &example());
    let output = Command::new("jco")
        .arg("wit")
        .arg(&component)
        .output()
        .unwrap();
    let wit = describe(&output);
    assert!(
        wit.contains("import maestro:extension/host@0.1.0;"),
        "{wit}"
    );
    assert!(
        wit.contains("export maestro:extension/guest@0.1.0;"),
        "{wit}"
    );
}

/// The installed component rewrites the prompt, is offered to the model as a tool and
/// answers the model's call, all through the host.
#[test]
#[ignore = "needs a Maestro executable in target/maestro and the world in target/wit"]
fn the_host_runs_the_component() {
    let scratch = Scratch::new("host");
    let component = build(&scratch.0, &example());
    let call = Turn::Call {
        tool: "hello",
        arguments: "{\"name\":\"Ada\"}",
    };

    let Run { requests, output } = host::run(&scratch.0, &[&component], PROMPT, &[call]);

    assert!(output.status.success(), "{}", describe(&output));
    assert_eq!(
        requests.len(),
        2,
        "the model is called before and after the tool"
    );
    assert_eq!(
        last_text(&requests[0]),
        &json!(format!("{PROMPT}{SUFFIX}")),
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
}

/// Input that another extension sends reaches the model as sent: the example's hook leaves
/// it alone, though it rewrites what the user types.
#[test]
#[ignore = "needs a Maestro executable in target/maestro and the world in target/wit"]
fn input_from_another_extension_reaches_the_model_unchanged() {
    let scratch = Scratch::new("probe");
    let example = build(&scratch.0, &example());
    let probe = build(&scratch.0, &root().join(PROBE));
    let call = Turn::Call {
        tool: "probe",
        arguments: "{}",
    };

    let Run { requests, output } = host::run(&scratch.0, &[&example, &probe], PROMPT, &[call]);

    assert!(output.status.success(), "{}", describe(&output));
    assert_eq!(
        requests.len(),
        3,
        "the model is called before the tool, after it and for the queued message"
    );
    assert_eq!(last_text(&requests[2]), &json!(SENT));
}
