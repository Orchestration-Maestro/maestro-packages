//! The shipped modal editor loaded into the production engine.
use super::lane_c::{Dialogs, emit};
use super::support::{harness, load, local};
use maestro_extensions::{SessionStartEvent, types::EditorFactoryOutcome};
use serde_json::{Value, json};
use std::{cell::Cell, rc::Rc};

/// The variable that names the built modal-editor component.
const COMPONENT: &str = "MAESTRO_MODAL_EDITOR_COMPONENT";

/// The escape byte.
const ESCAPE: &str = "\x1b";

/// A started session and the editor its session start installed.
struct Started {
    /// Keeps the engine alive.
    _harness: maestro_extensions_wasmtime::test_support::Harness,
    /// The editor the factory built.
    editor: EditorFactoryOutcome,
    /// How many times the application's interrupt callback ran.
    interrupts: Rc<Cell<u32>>,
    /// The UI that received the editor.
    ui: Rc<Dialogs>,
}

/// Starts the example and delivers one session start.
async fn started() -> Started {
    let harness = harness();
    let loaded = load(&harness, COMPONENT).await;
    let ui = Dialogs::install(&loaded.runner);
    let event = json!({"type": "session_start", "reason": "new"});
    assert_eq!(
        emit::<SessionStartEvent>(&loaded.runner, &event).await,
        Ok(Value::Null)
    );
    let editor = ui.editor().expect("the session start installed an editor");
    let interrupts = Rc::new(Cell::new(0));
    let counter = interrupts.clone();
    editor
        .as_custom()
        .expect("a custom editor")
        .set_on_escape(Some(Rc::new(move || counter.set(counter.get() + 1))));
    Started {
        _harness: harness,
        editor,
        interrupts,
        ui,
    }
}

impl Started {
    /// Types each input into the editor.
    fn type_all(&self, inputs: &[&str]) {
        for data in inputs {
            self.editor.try_handle_input(data).unwrap();
        }
    }

    /// The editor's text.
    fn text(&self) -> String {
        self.editor.get_text()
    }

    /// The editor's rows at `width`, with styling removed.
    fn rows(&self, width: usize) -> Vec<String> {
        self.editor
            .try_render(width)
            .unwrap()
            .iter()
            .map(|row| maestro_app::strip_ansi(row).unwrap())
            .collect()
    }
}

/// Each session start replaces the host's editor with one built on the custom editor.
#[test]
fn modal_editor_session_start_installs_a_custom_editor() {
    local(async {
        let started = started().await;
        assert!(started.editor.as_custom().is_some());
        assert!(
            started
                .ui
                .take()
                .iter()
                .any(|request| request == &json!(["setEditorComponent", [true]]))
        );
    });
}

/// Escape enters normal mode once and then reaches the editor; `i` returns to insert mode.
#[test]
fn modal_editor_escape_enters_normal_mode_and_then_interrupts() {
    local(async {
        let started = started().await;
        started.type_all(&["q", ESCAPE, "q", ESCAPE]);
        assert_eq!(
            (started.text().as_str(), started.interrupts.get()),
            ("q", 1)
        );
        started.type_all(&["i", "q", ESCAPE]);
        assert_eq!(
            (started.text().as_str(), started.interrupts.get()),
            ("qq", 1)
        );
    });
}

/// Normal-mode keys send the editor's navigation and deletion input; `a` moves right first.
#[test]
fn modal_editor_normal_keys_send_navigation_input() {
    local(async {
        let started = started().await;
        started.type_all(&["h", "e", "l", "l", "o", ESCAPE]);
        started.type_all(&["0", "x"]);
        assert_eq!(
            started.text(),
            "ello",
            "0 goes to the line start and x deletes the character"
        );
        started.type_all(&["$", "h", "a", "!"]);
        assert_eq!(
            started.text(),
            "ello!",
            "$ goes to the end, h moves left and a appends"
        );
        started.type_all(&[ESCAPE, "0", "l", "i", "-"]);
        assert_eq!(
            started.text(),
            "e-llo!",
            "l moves right and i inserts there"
        );
    });
}

/// Normal mode passes control input and multi-character input on, and drops one printable
/// character, astral and delete included.
#[test]
fn modal_editor_normal_mode_passes_control_input_and_drops_characters() {
    local(async {
        let started = started().await;
        started.type_all(&[ESCAPE, "q", "é", "😀", "\x7f"]);
        assert_eq!(started.text(), "", "one printable character is dropped");
        started.type_all(&["paste"]);
        assert_eq!(
            started.text(),
            "paste",
            "multi-character input reaches the editor"
        );
    });
}

/// The bottom line ends with the mode, and stays as it is when narrower than the label.
#[test]
fn modal_editor_bottom_line_shows_the_mode() {
    local(async {
        let started = started().await;
        started.type_all(&["abc"]);
        assert!(started.rows(20).last().unwrap().ends_with(" INSERT "));
        started.type_all(&[ESCAPE]);
        assert!(started.rows(20).last().unwrap().ends_with(" NORMAL "));
        assert!(!started.rows(4).last().unwrap().contains("NORMAL"));
    });
}

/// The editor keeps every operation of the custom editor it extends.
#[test]
fn modal_editor_keeps_the_custom_editor_operations() {
    local(async {
        let started = started().await;
        let editor = &started.editor;
        editor.set_text("a");
        assert_eq!(editor.insert_text_at_cursor("!"), Some(()));
        assert_eq!(editor.get_text(), "a!");
        assert_eq!(editor.get_expanded_text().as_deref(), Some("a!"));
        assert!(editor.border_color().is_some());
        assert_eq!(editor.set_padding_x(2.0), Some(()));
        assert_eq!(editor.add_to_history("past"), Some(()));
        editor.set_on_change(Some(Rc::new(|_| {})));
        assert!(editor.on_change().is_some());
        editor.set_on_submit(Some(Rc::new(|_| {})));
        assert!(editor.on_submit().is_some());
    });
}
