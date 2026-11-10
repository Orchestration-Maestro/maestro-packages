//! The shipped questionnaire tool loaded into the production engine.
//!
//! The recorded frames were drawn with tagged stand-ins for colors and clipping, so each frame
//! is compared after the tags are removed, at the 20 columns the recording used.
use super::lane_a::{Ui, render_call, render_result};
use super::support::{call_tool, harness, load, local};
use serde_json::{Value, json};

/// The variable that names the built questionnaire component.
const COMPONENT: &str = "MAESTRO_QUESTIONNAIRE_COMPONENT";

/// The recorded rows these tests consume.
fn corpus() -> Value {
    serde_json::from_str(include_str!("questionnaire_expected.json")).unwrap()
}

/// The rows of one recorded behavior group.
fn group(name: &str) -> Vec<Value> {
    let rows: Vec<Value> = corpus()["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["group"] == name)
        .cloned()
        .collect();
    assert!(!rows.is_empty(), "recorded rows of {name}");
    rows
}

/// The recorded row named `name`.
fn row(name: &str) -> Value {
    corpus()["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["name"] == name)
        .unwrap()
        .clone()
}

/// The terminal input a recorded key name stands for; any other entry is typed text.
fn press(name: &str) -> &str {
    match name {
        "enter" => "\r",
        "up" => "\x1b[A",
        "down" => "\x1b[B",
        "left" => "\x1b[D",
        "right" => "\x1b[C",
        "escape" => "\x1b",
        "tab" => "\t",
        "shift+tab" => "\x1b[Z",
        text => text,
    }
}

/// A recorded text without its clipping markers and color tags.
fn untagged(recorded: &str) -> String {
    let mut plain = String::new();
    let mut rest = recorded;
    while let Some(c) = rest.chars().next() {
        if let Some(after) = rest.strip_prefix("clip|") {
            let digits = after.chars().take_while(char::is_ascii_digit).count();
            if digits > 0 && after[digits..].starts_with('|') {
                rest = &after[digits + 1..];
                continue;
            }
        }
        if c == '<' {
            let tag = rest[1..]
                .chars()
                .take_while(char::is_ascii_alphabetic)
                .count();
            if tag > 0 && rest[1 + tag..].starts_with('>') {
                rest = &rest[tag + 2..];
                continue;
            }
        }
        plain.push(c);
        rest = &rest[c.len_utf8()..];
    }
    plain
}

/// A recorded row as the terminal draws it: the stand-in asked for clipping to a width, which
/// the real helper does by ending an overlong row in an ellipsis.
fn clipped(recorded: &str) -> String {
    let width = recorded
        .strip_prefix("clip|")
        .and_then(|rest| rest.split_once('|'))
        .and_then(|(width, _)| width.parse::<usize>().ok());
    let text = untagged(recorded);
    match width {
        Some(width) if text.chars().count() > width => {
            let kept: String = text.chars().take(width - 3).collect();
            format!("{kept}...")
        }
        _ => text,
    }
}

/// A row without the blanks the terminal pads it with.
fn ended(row: &str) -> String {
    row.trim_end_matches([' ', '\t']).to_owned()
}

/// A frame of the real editor, drawn as the recording drew its stand-in: the editor's top border
/// becomes the stand-in's marker and its bottom border is dropped.
fn as_recorded(frame: &[String]) -> Vec<String> {
    let border = |row: &str| row.starts_with(" \u{2500}");
    let mut rows = Vec::new();
    let mut editing = 0;
    for row in frame {
        let row = ended(&super::lane_a::plain(row));
        match (editing, border(&row)) {
            (0, false) if row == " Your answer:" => editing = 1,
            (1, true) => {
                rows.push(" edit|18".to_owned());
                editing = 2;
                continue;
            }
            (2, true) => {
                editing = 0;
                continue;
            }
            _ => {}
        }
        rows.push(row);
    }
    rows
}

/// Runs one recorded row: its questions, its key presses and its UI, or none when headless.
async fn replay(row: &Value) {
    let harness = harness();
    let loaded = load(&harness, COMPONENT).await;
    let ui = row["ui"].as_bool().unwrap().then(|| {
        let ui = Ui::install_with_screen(&loaded.runner);
        for input in row["inputs"].as_array().unwrap() {
            ui.press(press(input.as_str().unwrap()));
        }
        ui
    });
    let name = &row["name"];
    let answered = call_tool(
        &loaded.runner,
        "questionnaire",
        json!({"questions": row["questions"]}),
    )
    .await;
    assert_eq!(answered, Ok(row["result"].clone()), "{name}");
    let recorded: Vec<Vec<String>> = row["frames"]
        .as_array()
        .unwrap()
        .iter()
        .map(|frame| {
            frame
                .as_array()
                .unwrap()
                .iter()
                .map(|line| ended(&clipped(line.as_str().unwrap())))
                .collect()
        })
        .collect();
    let shown: Vec<Vec<String>> = ui.map_or_else(Vec::new, |ui| {
        ui.frames().iter().map(|frame| as_recorded(frame)).collect()
    });
    assert_eq!(shown, recorded, "{name}: frames");
}

/// Replays every row of a recorded group.
fn replay_group(name: &str) {
    local(async {
        for row in group(name) {
            replay(&row).await;
        }
    });
}

/// Extra schema-permitted question and option fields survive in structured details.
#[test]
fn questionnaire_retains_supplied_question_metadata() {
    replay_group("metadata");
}

/// Headless and empty input return cancelled details without opening custom UI.
#[test]
fn questionnaire_headless_and_empty_return_errors() {
    replay_group("headless");
}

/// Option navigation clamps and one question submits the selected one-based answer immediately.
#[test]
fn questionnaire_selected_answer_finishes_single_question() {
    replay_group("selected");
}

/// Escape discards editor text; resubmission uses shared whitespace and the blank fallback.
#[test]
fn questionnaire_custom_answers_use_inline_editor() {
    replay_group("custom");
}

/// An empty identifier is a string value, not an absent inline-editor question.
#[test]
fn questionnaire_custom_answer_accepts_empty_identifier() {
    local(async {
        let mut blank = row("blank");
        blank["name"] = json!("empty-identifier");
        blank["questions"][0]["id"] = json!("");
        blank["result"]["details"]["questions"][0]["id"] = json!("");
        blank["result"]["details"]["answers"][0]["id"] = json!("");
        replay(&blank).await;
    });
}

/// Tabs wrap, unanswered submission stays open, and revising answers retains insertion order.
#[test]
fn questionnaire_tabs_require_all_answers_and_allow_revisions() {
    replay_group("tabs");
}

/// Cancellation works from a question and Submit, retaining partial answers.
#[test]
fn questionnaire_cancellation_retains_partial_answers() {
    replay_group("cancel");
}

/// A question with no selectable options ignores Enter rather than failing, and stays cancellable.
#[test]
fn questionnaire_empty_option_list_remains_cancellable() {
    local(async {
        let mut partial = row("partial");
        partial["inputs"]
            .as_array_mut()
            .unwrap()
            .insert(1, json!("enter"));
        let unchanged = partial["frames"][1].clone();
        partial["frames"]
            .as_array_mut()
            .unwrap()
            .insert(2, unchanged);
        replay(&partial).await;
    });
}

/// Both renderers preserve counts, supplied labels, cancellation and custom/selected summaries.
#[test]
fn questionnaire_renderers_show_call_and_answer_summaries() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let runner = &loaded.runner;
        for row in corpus()["rows"].as_array().unwrap() {
            let args = json!({"questions": row["questions"]});
            let call = render_call(runner, "questionnaire", &args);
            assert_eq!(
                call.join("\n"),
                untagged(row["call"].as_str().unwrap()),
                "{}",
                row["name"]
            );
            let result = render_result(runner, "questionnaire", &args, &row["result"], [false; 2]);
            assert_eq!(
                result.join("\n"),
                untagged(row["rendered"].as_str().unwrap()),
                "{}",
                row["name"]
            );
        }
        let fallback = json!({"content": [{"type": "text", "text": "fallback"}]});
        let shown = render_result(runner, "questionnaire", &json!({}), &fallback, [false; 2]);
        assert_eq!(shown, ["fallback"]);
    });
}
