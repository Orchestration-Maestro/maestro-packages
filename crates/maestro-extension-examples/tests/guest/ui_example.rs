//! The shipped question tool's interactive view, driven through the terminal input it reads.
use super::lane_b::{Instance, Recorder, driven};
use super::support::call_tool;
use serde_json::{Value, json};
use std::{collections::HashMap, rc::Rc};

/// The variable that names the built question component.
const COMPONENT: &str = "MAESTRO_QUESTION_COMPONENT";

/// An up-arrow key.
const UP: &str = "\x1b[A";
/// A down-arrow key.
const DOWN: &str = "\x1b[B";
/// The enter key.
const ENTER: &str = "\r";
/// The escape key.
const ESCAPE: &str = "\x1b";

/// The recorded frames of the choice list and the editor, by state name.
fn frames() -> HashMap<String, Vec<String>> {
    serde_json::from_str(include_str!("ui_frames.json")).unwrap()
}

/// The same question with one described choice and one plain choice.
fn params() -> Value {
    json!({"question":"Which?","options":[{"label":"Rust","description":"Fast"},{"label":"Go"}]})
}

/// Asserts complete result records, including omitted versus present custom flags.
fn answer(actual: &Result<Value, String>, text: &str, answer: &Value, custom: Option<bool>) {
    let mut details = json!({"question":"Which?","options":["Rust","Go"],"answer":answer});
    if let Some(custom) = custom {
        details["wasCustom"] = json!(custom);
    }
    assert_eq!(
        actual,
        &Ok(json!({"content":[{"type":"text","text":text}],"details":details}))
    );
}

/// One question being asked: the running tool call and the view it opened.
struct Asking {
    /// The UI the view was shown on.
    ui: Rc<Recorder>,
    /// The running tool call.
    call: tokio::task::JoinHandle<Result<Value, String>>,
}

impl Asking {
    /// Starts the tool on `instance` and waits for its view.
    async fn start(instance: &Instance, params: Value) -> Self {
        let ui = Recorder::install_interactive(instance.runner(), 80, 24);
        let runner = instance.runner().clone();
        let call =
            tokio::task::spawn_local(async move { call_tool(&runner, "question", params).await });
        ui.shown(1).await;
        Self { ui, call }
    }

    /// Sends `data` to the view as terminal input.
    fn press(&self, data: &str) {
        self.ui.press(0, data);
    }

    /// Types `text` one character at a time.
    fn type_text(&self, text: &str) {
        for ch in text.chars() {
            self.press(&ch.to_string());
        }
    }

    /// Asserts the view shows the recorded frame `name`.
    ///
    /// The choice frames match whole. The editor frames match above and below the editor, whose
    /// rows are the application's own.
    fn shows(&self, name: &str) {
        let frame = frames().remove(name).unwrap();
        let rows = self.ui.rows(0, 80);
        if !matches!(name, "editing" | "typed" | "blank") {
            assert_eq!(rows, frame, "{name}");
            return;
        }
        let head = frame.iter().position(|row| row == " Your answer:").unwrap() + 1;
        assert_eq!(rows[..head], frame[..head], "{name}: above the editor");
        let tail = &frame[frame.len() - 3..];
        assert_eq!(rows[rows.len() - 3..], *tail, "{name}: below the editor");
    }

    /// Waits for the tool call to finish and returns its outcome.
    async fn finished(self) -> Result<Value, String> {
        self.call.await.unwrap()
    }
}

/// Navigation clamps, editor Escape clears, blank submit returns, and choices keep numbering.
#[test]
fn question_ui_navigates_edits_and_selects_a_choice() {
    driven(async {
        let instance = Instance::new(COMPONENT).await;
        let asking = Asking::start(&instance, params()).await;
        asking.shows("first");
        for (key, state) in [
            (UP, "first"),
            (DOWN, "second"),
            (DOWN, "custom"),
            (DOWN, "custom"),
        ] {
            asking.press(key);
            asking.shows(state);
        }
        asking.press(ENTER);
        asking.shows("editing");
        asking.type_text("  snow  ");
        asking.shows("typed");
        assert!(asking.ui.rows(0, 80).iter().any(|row| row.contains("snow")));
        asking.press(ESCAPE);
        asking.shows("custom");
        asking.press(ENTER);
        asking.shows("editing");
        asking.type_text(" ");
        asking.shows("blank");
        asking.press(ENTER);
        asking.shows("custom");
        asking.press(UP);
        asking.shows("second");
        asking.press(ENTER);
        answer(
            &asking.finished().await,
            "User selected: 2. Go",
            &json!("Go"),
            Some(false),
        );
    });
}

/// A trimmed custom answer returns, Escape cancels, and a question without options is an error.
#[test]
fn question_ui_customizes_and_cancels() {
    driven(async {
        let instance = Instance::new(COMPONENT).await;
        let asking = Asking::start(&instance, params()).await;
        asking.press(DOWN);
        asking.press(DOWN);
        asking.press(ENTER);
        asking.type_text("  snow  ");
        asking.press(ENTER);
        answer(
            &asking.finished().await,
            "User wrote: snow",
            &json!("snow"),
            Some(true),
        );
        let cancelled = Asking::start(&instance, params()).await;
        cancelled.press(ESCAPE);
        answer(
            &cancelled.finished().await,
            "User cancelled the selection",
            &Value::Null,
            None,
        );
        let empty = call_tool(
            instance.runner(),
            "question",
            json!({"question":"Which?","options":[]}),
        )
        .await;
        assert_eq!(
            empty,
            Ok(
                json!({"content":[{"type":"text","text":"Error: No options provided"}],"details":{"question":"Which?","options":[],"answer":null}})
            )
        );
    });
}
