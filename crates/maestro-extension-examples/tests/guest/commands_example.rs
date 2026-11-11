//! The shipped command selector loaded into the production engine.
use super::support::{Failures, Recorder, harness, load, local};
use maestro_extensions_wasmtime::test_support::maestro_extensions::{
    AutocompleteItem, SlashCommandInfo,
};
use serde_json::{Value, json};
use std::rc::Rc;

/// The variable that names the built command-selector component.
const COMPONENT: &str = "MAESTRO_COMMANDS_COMPONENT";

/// Completion prefixes and their independent expected source labels.
fn completion_cases() -> Vec<(&'static str, Option<Vec<Value>>)> {
    [
        ("", Some(vec!["extension", "prompt", "skill"])),
        ("e", Some(vec!["extension"])),
        ("prompt", Some(vec!["prompt"])),
        ("s", Some(vec!["skill"])),
        ("extensions", None),
        ("E", None),
        (" skill", None),
    ]
    .into_iter()
    .map(|(prefix, labels)| {
        (
            prefix,
            labels.map(|labels| {
                labels
                    .into_iter()
                    .map(|label| json!({"value": label, "label": label, "description": null}))
                    .collect()
            }),
        )
    })
    .collect()
}

/// A completion item as the value, label and description it offers.
fn labelled(item: &AutocompleteItem) -> Value {
    json!({"value": item.value, "label": item.label, "description": item.description})
}

/// One command invocation with the host's inventory and the dialog answers.
struct Case {
    /// Argument text, including any surrounding whitespace.
    args: &'static str,
    /// Inventory the host reports for this invocation.
    inventory: Value,
    /// The selector's answer, and the confirmation's when the selection reaches one.
    answers: (Option<&'static str>, Option<bool>),
    /// Exact UI requests and operands the command must make.
    expected: Vec<(&'static str, Value)>,
}

/// Unsorted inventory with absent, empty and nonempty descriptions and paths.
fn inventory() -> Value {
    json!([
        {"name":"research", "description":"Find sources", "source":"skill",
         "sourceInfo":{"path":"./skills/research/SKILL.md", "source":"local", "scope":"project", "origin":"top-level"}},
        {"name":"greet", "description":"Hello 雪", "source":"extension",
         "sourceInfo":{"path":"../extensions/greet.rs", "source":"local", "scope":"user", "origin":"top-level"}},
        {"name":"draft", "description":"", "source":"prompt",
         "sourceInfo":{"path":"", "source":"local", "scope":"project", "origin":"top-level"}},
        {"name":"bare", "source":"extension",
         "sourceInfo":{"path":"/extensions/bare.rs", "source":"package", "scope":"user", "origin":"package"}}
    ])
}

/// The selector request for `options`.
fn select(options: &[&str]) -> (&'static str, Value) {
    ("select", json!(["Available Commands", options, null, null]))
}

/// The complete grouped selector, independent of the inventory's order.
fn selector() -> (&'static str, Value) {
    select(&[
        "--- Extensions ---",
        "/greet - Hello 雪",
        "/bare",
        "--- Prompts ---",
        "/draft",
        "--- Skills ---",
        "/research - Find sources",
    ])
}

/// A notification of `message` at the information level.
fn info(message: &str) -> (&'static str, Value) {
    ("notify", json!([message, "Info"]))
}

/// Exact source filters retain the host's order within each group.
fn filtered_cases() -> Vec<Case> {
    [
        (
            " \t extension\n",
            select(&["--- Extensions ---", "/greet - Hello 雪", "/bare"]),
        ),
        ("prompt", select(&["--- Prompts ---", "/draft"])),
        (
            "skill",
            select(&["--- Skills ---", "/research - Find sources"]),
        ),
    ]
    .into_iter()
    .map(|(args, expected)| Case {
        args,
        inventory: inventory(),
        answers: (None, None),
        expected: vec![expected],
    })
    .collect()
}

/// Empty matches, group headers and cancelled selection.
fn listing_cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for (args, supplied, expected) in [
        ("", json!([]), vec![info("No commands found")]),
        ("", inventory(), vec![selector()]),
        (
            "extensions",
            inventory(),
            vec![info("No extensions commands found")],
        ),
        (
            "Extension",
            inventory(),
            vec![info("No Extension commands found")],
        ),
        ("skill", json!([]), vec![info("No skill commands found")]),
    ] {
        cases.push(Case {
            args,
            inventory: supplied,
            answers: (None, None),
            expected,
        });
    }
    cases.extend(filtered_cases());
    for answer in ["--- Extensions ---", "", "/missing", "/draft"] {
        cases.push(Case {
            args: "",
            inventory: inventory(),
            answers: (Some(answer), None),
            expected: vec![selector()],
        });
    }
    cases
}

/// Path notifications require affirmative confirmation.
fn path_cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for (selected, name, path) in [
        ("/greet - Hello 雪", "greet", "../extensions/greet.rs"),
        ("/bare", "bare", "/extensions/bare.rs"),
        (
            "/research - Find sources",
            "research",
            "./skills/research/SKILL.md",
        ),
    ] {
        for confirmed in [false, true] {
            let mut expected = vec![
                selector(),
                (
                    "confirm",
                    json!([name, format!("View source path?\n{path}"), null, null]),
                ),
            ];
            if confirmed {
                expected.push(info(path));
            }
            cases.push(Case {
                args: "",
                inventory: inventory(),
                answers: (Some(selected), Some(confirmed)),
                expected,
            });
        }
    }
    cases
}

/// Runs the command once per case against one loaded component and checks every UI request.
fn run(cases: &[Case]) {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let runner = &loaded.runner;
        let ui = Recorder::install(runner);
        let reported = Failures::collect(runner);
        let command = runner.get_command("commands").unwrap().command;
        for case in cases {
            let supplied: Vec<SlashCommandInfo> =
                serde_json::from_value(case.inventory.clone()).unwrap();
            harness.runtime.actions.write().unwrap().get_commands =
                Rc::new(move || Ok(supplied.clone()));
            ui.answer(json!(case.answers.0));
            ui.answer(json!(case.answers.1.unwrap_or(false)));
            (command.handler)(case.args.into(), runner.create_command_context())
                .await
                .unwrap();
            let seen: Vec<_> = ui
                .take()
                .into_iter()
                .map(|request| (request[0].as_str().unwrap().to_owned(), request[1].clone()))
                .collect();
            let expected: Vec<_> = case
                .expected
                .iter()
                .map(|(method, operands)| ((*method).to_owned(), operands.clone()))
                .collect();
            assert_eq!(seen, expected, "arguments {:?}", case.args);
        }
        assert!(reported.seen().is_empty());
    });
}

/// The command registers its description and completes the three source names.
#[test]
fn commands_registers_metadata_and_completes_sources() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let commands = loaded.runner.get_registered_commands();
        assert_eq!(commands.len(), 1);
        let command = &commands[0].command;
        assert_eq!(command.name, "commands");
        assert_eq!(
            command.description.as_deref(),
            Some("List available slash commands")
        );
        let complete = command.get_argument_completions.as_ref().unwrap();
        for (prefix, expected) in completion_cases() {
            let items = complete(prefix.into()).await.unwrap();
            let items = items.map(|items| items.iter().map(labelled).collect::<Vec<_>>());
            assert_eq!(items, expected, "prefix {prefix:?}");
        }
    });
}

/// The selector lists the filtered inventory by source and ignores cancelled or unusable picks.
#[test]
fn commands_lists_filtered_inventory_and_handles_cancellation() {
    run(&listing_cases());
}

/// A source path is shown only after the user confirms it.
#[test]
fn commands_notifies_source_paths_only_after_confirmation() {
    run(&path_cases());
}
