//! The shipped bookmark commands loaded into the production engine over real session history.
//!
//! The recorded cases name entries by short identities; each case builds its entries in a real
//! session, whose own identities map back to those names when labels are compared. A refused
//! entry read or label read, and the injected clock, have no public form on the session or the
//! guest clock, so those cases check what a real session shows.
use super::lane_a::{Ui, failures};
use super::support::{harness, load, local};
use maestro_extensions_wasmtime::test_support::{Harness, Loaded};
use maestro_models::UserContent;
use maestro_session::{Presence, SessionManager, session_manager::CompactionOptions};
use serde_json::{Value, json};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

/// The variable that names the built bookmark component.
const COMPONENT: &str = "MAESTRO_BOOKMARK_COMPONENT";

/// The recorded registration and case rows these tests consume.
fn corpus() -> Value {
    serde_json::from_str(include_str!("bookmark_expected.json")).unwrap()
}

/// The recorded rows `keep` selects.
fn rows(keep: impl Fn(&Value) -> bool) -> Vec<Value> {
    let rows: Vec<Value> = corpus()["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| keep(row))
        .cloned()
        .collect();
    assert!(!rows.is_empty(), "recorded rows");
    rows
}

/// Whether the row's failure, if any, is one a real session and UI can produce.
fn reproducible(row: &Value) -> bool {
    matches!(row["failure"].as_str(), None | Some("setLabel" | "notify"))
}

/// A session built from a recorded row, the component over it and what the component did.
struct Case {
    /// The engine the component lives in.
    harness: Harness,
    /// The runner over the component.
    loaded: Loaded,
    /// The notifications the commands showed.
    ui: Rc<Ui>,
    /// Each label write as the recorded name and the label, absent when cleared.
    sets: Rc<RefCell<Vec<Value>>>,
    /// The callback failures the runner reported.
    failures: Rc<RefCell<Vec<String>>>,
    /// The recorded names of the real entry identities.
    names: Rc<RefCell<BTreeMap<String, String>>>,
}

impl Case {
    /// Loads the component over the entries and labels the recorded row supplies.
    async fn new(row: &Value) -> Self {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let ui = Ui::install(&loaded.runner);
        let reported = failures(&loaded.runner);
        let names = Rc::new(RefCell::new(BTreeMap::new()));
        let sets = Rc::new(RefCell::new(Vec::new()));
        let ids = build_entries(&mut harness.session_manager.borrow_mut(), row, &names);
        let session = harness.session_manager.clone();
        let (writes, recorded, refusal) = (
            sets.clone(),
            names.clone(),
            row["failure"].as_str() == Some("setLabel"),
        );
        harness.runtime.actions.write().unwrap().set_label = Rc::new(move |id, label| {
            if refusal {
                return Err("host refused setLabel".into());
            }
            let name = recorded.borrow()[&id].clone();
            let write = label.as_deref().map_or(
                json!({"id": name}),
                |label| json!({"id": name, "label": label}),
            );
            writes.borrow_mut().push(write);
            let presence = label
                .as_deref()
                .map_or(Presence::Missing, Presence::Present);
            session
                .borrow_mut()
                .append_label_change(&id, presence)
                .map(drop)
                .map_err(|e| e.to_string().into())
        });
        for (name, label) in row["labels"].as_object().unwrap() {
            let id = &ids[name];
            let label = label.as_str().unwrap();
            if !label.is_empty() {
                let presence = Presence::Present(label);
                harness
                    .session_manager
                    .borrow_mut()
                    .append_label_change(id, presence)
                    .unwrap();
            }
        }
        if row["failure"].as_str() == Some("notify") {
            ui.reject("notify", "host refused notify");
        }
        Self {
            harness,
            loaded,
            ui,
            sets,
            failures: reported,
            names,
        }
    }

    /// Runs the recorded command and returns its error, if any.
    async fn run(&self, row: &Value) -> Option<String> {
        let command = self
            .loaded
            .runner
            .get_command(row["command"].as_str().unwrap())
            .unwrap()
            .command;
        (command.handler)(
            row["args"].as_str().unwrap().into(),
            self.loaded.runner.create_command_context(),
        )
        .await
        .err()
        .map(|error| error.to_string())
    }
}

/// Appends the row's entries to `session`, records each real identity under its recorded name and
/// returns the identities by recorded name.
fn build_entries(
    session: &mut SessionManager,
    row: &Value,
    names: &RefCell<BTreeMap<String, String>>,
) -> BTreeMap<String, String> {
    let mut ids: BTreeMap<String, String> = BTreeMap::new();
    for entry in row["entries"].as_array().unwrap() {
        let id = match entry["type"].as_str().unwrap() {
            "message" => session.append_message(entry["message"].clone()).unwrap(),
            "thinking_level_change" => session
                .append_thinking_level_change(entry["thinkingLevel"].as_str().unwrap())
                .unwrap(),
            "model_change" => session
                .append_model_change(
                    entry["provider"].as_str().unwrap(),
                    entry["modelId"].as_str().unwrap(),
                )
                .unwrap(),
            "compaction" => session
                .append_compaction(
                    entry["summary"].as_str().unwrap(),
                    "kept",
                    entry["tokensBefore"].as_f64().unwrap(),
                    CompactionOptions::default(),
                )
                .unwrap(),
            "branch_summary" => session
                .branch_with_summary(
                    None,
                    entry["summary"].as_str().unwrap(),
                    Presence::Missing,
                    Presence::Missing,
                )
                .unwrap(),
            "custom" => session
                .append_custom_entry(entry["customType"].as_str().unwrap(), Presence::Missing)
                .unwrap(),
            "custom_message" => session
                .append_custom_message_entry(
                    entry["customType"].as_str().unwrap(),
                    UserContent::Text(entry["content"].as_str().unwrap().into()),
                    true,
                    Presence::Missing,
                )
                .unwrap(),
            "label" => {
                // The recorded target may not be among the entries; any earlier entry stands in.
                let target = match ids
                    .get(entry["targetId"].as_str().unwrap())
                    .or(ids.values().next())
                {
                    Some(target) => target.clone(),
                    None => session.append_session_info("target").unwrap(),
                };
                session
                    .append_label_change(
                        &target,
                        Presence::Present(entry["label"].as_str().unwrap()),
                    )
                    .unwrap()
            }
            _ => session
                .append_session_info(entry["name"].as_str().unwrap())
                .unwrap(),
        };
        let name = entry["id"].as_str().unwrap().to_owned();
        names.borrow_mut().insert(id.clone(), name.clone());
        ids.insert(name, id);
    }
    ids
}

/// Runs one recorded row and checks the label writes, notifications and error against it.
async fn replay(row: &Value) {
    let case = Case::new(row).await;
    let error = case.run(row).await;
    let expected = &row["expected"];
    let notices: Vec<Value> = case
        .ui
        .take()
        .iter()
        .map(|request| json!({"message": request["operands"][0], "kind": request["operands"][1].as_str().map(str::to_lowercase)}))
        .collect();
    let recorded: Vec<Value> = expected["notifications"]
        .as_array()
        .unwrap()
        .iter()
        .map(|notice| json!({"message": notice["message"], "kind": notice["kind"]}))
        .collect();
    let name = &row["name"];
    assert_eq!(notices, recorded, "{name}: notifications");
    let sets = case.sets.borrow().clone();
    let defaulted = expected["clockReads"] == 1 && !sets.is_empty();
    if !defaulted {
        assert_eq!(Value::Array(sets), expected["sets"], "{name}: label writes");
    }
    assert_eq!(
        error.as_deref(),
        expected["error"].as_str(),
        "{name}: error"
    );
    assert!(case.failures.borrow().is_empty(), "{name}");
    let _ = (&case.harness, &case.names);
}

/// Replays every selected row.
fn replay_all(selected: &[Value]) {
    local(async {
        for row in selected {
            replay(row).await;
        }
    });
}

/// Exactly the recorded command registrations, and no tool.
#[test]
fn bookmark_registers_command_metadata() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let commands = loaded.runner.get_registered_commands();
        let seen: Vec<Value> = commands
            .iter()
            .map(|registered| {
                json!({
                    "name": registered.command.name,
                    "description": registered.command.description,
                    "completions": registered.command.get_argument_completions.is_some(),
                })
            })
            .collect();
        assert_eq!(Value::Array(seen), corpus()["metadata"]);
        assert!(loaded.runner.get_all_registered_tools().is_empty());
    });
}

/// Every recorded explicit-label case labels the newest assistant message.
#[test]
fn bookmark_selects_latest_assistant() {
    replay_all(&rows(|row| {
        row["command"] == "bookmark"
            && reproducible(row)
            && row["failure"].is_null()
            && row["expected"]["clockReads"] == 0
            && !row["expected"]["sets"].as_array().unwrap().is_empty()
            && !["trim", "space"]
                .iter()
                .any(|word| row["name"].as_str().unwrap().contains(word))
    }));
}

/// A session without an eligible assistant message is reported and left unlabelled.
#[test]
fn bookmark_missing_assistant_warns() {
    replay_all(&rows(|row| {
        row["command"] == "bookmark"
            && row["failure"].is_null()
            && row["expected"]["sets"].as_array().unwrap().is_empty()
            && row["expected"]["notifications"][0]["kind"] == "warning"
    }));
}

/// Every recorded removal case clears the newest label of any entry kind.
#[test]
fn unbookmark_selects_latest_nonempty_label() {
    replay_all(&rows(|row| {
        row["command"] == "unbookmark"
            && row["failure"].is_null()
            && row["expected"]["notifications"][0]["kind"] == "info"
    }));
}

/// A session without a label anywhere is reported and left alone.
#[test]
fn unbookmark_without_labels_warns() {
    replay_all(&rows(|row| {
        row["command"] == "unbookmark"
            && row["failure"].is_null()
            && row["expected"]["notifications"][0]["kind"] == "warning"
    }));
}

/// A refused label write or notification keeps the effects that came before it.
#[test]
fn bookmark_host_failures_preserve_effect_order() {
    replay_all(&rows(|row| {
        matches!(row["failure"].as_str(), Some("setLabel" | "notify"))
    }));
}

/// A long session still finds the one labelled entry at its far end.
#[test]
fn bookmark_walk_finds_labels_in_long_sessions() {
    local(async {
        for count in [32_usize, 128] {
            let entries: Vec<Value> = (0..count)
                .map(
                    |index| json!({"type": "session_info", "id": format!("e{index}"), "name": "n"}),
                )
                .collect();
            let row = json!({
                "command": "unbookmark", "args": "", "entries": entries,
                "labels": {"e0": "deep"}, "failure": null,
                "expected": {"sets": [{"id": "e0"}], "error": null, "clockReads": 0,
                    "notifications": [{"message": "Removed bookmark: deep", "kind": "info"}]},
                "name": format!("deep-{count}"),
            });
            replay(&row).await;
        }
    });
}

/// Labels use the host's whitespace: ASCII and Unicode edges go, interior spaces stay.
#[test]
fn bookmark_labels_use_native_whitespace() {
    replay_all(&rows(|row| {
        ["ascii-trim", "unicode-trim", "internal-space"].contains(&row["name"].as_str().unwrap())
    }));
}

/// An empty or blank argument labels with the host clock's milliseconds.
#[test]
fn bookmark_component_reads_host_clock() {
    local(async {
        for row in rows(|row| {
            row["command"] == "bookmark"
                && row["expected"]["clockReads"] == 1
                && !row["expected"]["sets"].as_array().unwrap().is_empty()
        }) {
            let before = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis();
            let case = Case::new(&row).await;
            assert_eq!(case.run(&row).await, None);
            let after = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis();
            let sets = case.sets.borrow();
            let label = sets[0]["label"].as_str().unwrap();
            let millis: u128 = label.strip_prefix("bookmark-").unwrap().parse().unwrap();
            assert!((before..=after).contains(&millis), "{label}");
            assert_eq!(
                case.ui.take()[0]["operands"][0],
                format!("Bookmarked as: {label}")
            );
        }
    });
}
