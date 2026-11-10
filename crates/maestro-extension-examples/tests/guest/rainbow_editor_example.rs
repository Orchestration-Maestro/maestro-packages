//! The shipped rainbow editor loaded into the production engine over a started terminal.
//!
//! The recorded frames were drawn over a stand-in editor; here the real editor's rows must
//! contain the recorded highlighted text. Frames advance on the real 60 ms clock and each one
//! announces itself through the terminal's redraw, which the tests wait for. That the frames
//! stop once the editor is released has no signal to wait for, so it is not asserted.
use super::support::{Failures, Recorder, TERMINAL, drain, driven, event, harness, load};
use maestro_extensions_wasmtime::test_support::{
    Harness, Loaded, maestro_extensions::SessionStartEvent,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// The variable that names the built editor component.
const COMPONENT: &str = "MAESTRO_RAINBOW_EDITOR_COMPONENT";
/// Terminal input that moves the cursor and leaves the text alone.
const RIGHT: &str = "\x1b[C";

/// The recorded rows and keyword answers.
fn corpus() -> Value {
    serde_json::from_str(include_str!("rainbow_editor_expected.json")).unwrap()
}

/// The typed text whose rows the recorded frames highlight.
fn typed() -> String {
    let lines = &corpus()["lines"];
    format!(
        "{}\n{}",
        lines[1].as_str().unwrap(),
        lines[2].as_str().unwrap()
    )
}

/// A loaded component whose session start installed its editor on a started terminal.
struct Rainbow {
    /// The engine the component lives in.
    harness: Harness,
    /// The runner over the component.
    loaded: Loaded,
    /// The UI holding the installed editor and the terminal.
    ui: Rc<Recorder>,
    /// The callback failures the runner reported.
    failures: Failures,
}

impl Rainbow {
    /// Loads the component and starts a session.
    async fn new() -> Self {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let ui = Recorder::install_on(&loaded.runner, TERMINAL);
        let failures = Failures::collect(&loaded.runner);
        let mut start: SessionStartEvent =
            event(json!({"type": "session_start", "reason": "startup"}));
        loaded.runner.emit(&mut start).await.unwrap();
        assert!(failures.seen().is_empty());
        Self {
            harness,
            loaded,
            ui,
            failures,
        }
    }

    /// Sets the editor's text and then gives it input, as typing does.
    fn type_text(&self, text: &str) {
        let editor = self.ui.editor().unwrap().editor();
        editor.set_text(text);
        editor.handle_input(RIGHT);
    }

    /// The rows the editor renders 40 columns wide.
    fn rows(&self) -> Vec<String> {
        self.ui.editor().unwrap().editor().try_render(40).unwrap()
    }

    /// Waits for the animation's next frame and lets the terminal draw it.
    async fn next_frame(&self) {
        let screen = self.ui.stage();
        screen.redraws.notified().await;
        screen.runtime.settle().unwrap();
    }

    /// Starts the animation on the recorded text and clears redraws made by typing it.
    fn start_animation(&self) {
        self.type_text(&typed());
        let screen = self.ui.stage();
        screen.runtime.settle().unwrap();
        drain(&screen.redraws);
    }
}

/// Whether any row carries the example's rainbow colors; the editor's own border styling uses
/// other colors.
fn has_rainbow(rows: &[String]) -> bool {
    const COLORS: [&str; 7] = [
        "\x1b[38;2;233;137;115m",
        "\x1b[38;2;228;186;103m",
        "\x1b[38;2;141;192;122m",
        "\x1b[38;2;102;194;179m",
        "\x1b[38;2;121;157;207m",
        "\x1b[38;2;157;134;195m",
        "\x1b[38;2;206;130;172m",
    ];
    rows.iter()
        .any(|row| COLORS.iter().any(|color| row.contains(color)))
}

/// Whether `rows` hold the two recorded highlighted lines of `frame`.
fn shows_frame(rows: &[String], frame: usize) -> bool {
    let recorded = &corpus()["frames"][frame];
    [1, 2].iter().all(|line| {
        let expected = recorded[*line].as_str().unwrap();
        rows.iter().any(|row| row.contains(expected))
    })
}

/// Each 60 ms frame moves the shine over ten characters, pauses ten frames, then wraps.
#[test]
fn rainbow_editor_shine_follows_reference_frames() {
    driven(async {
        let rainbow = Rainbow::new().await;
        assert!(rainbow.loaded.runner.has_handlers("session_start"));
        rainbow.start_animation();
        for frame in 0..21 {
            let rows = rainbow.rows();
            assert!(shows_frame(&rows, frame), "frame {frame}: {rows:?}");
            rainbow.next_frame().await;
        }
        assert!(rainbow.failures.seen().is_empty());
        drop(rainbow.harness);
    });
}

/// Only input checks the text, matching the keyword in any ASCII case.
#[test]
fn rainbow_editor_input_starts_animation_on_keyword() {
    driven(async {
        for entry in corpus()["keyword"].as_array().unwrap() {
            let (text, animates) = (entry[0].as_str().unwrap(), entry[1].as_bool().unwrap());
            let rainbow = Rainbow::new().await;
            let editor = rainbow.ui.editor().unwrap().editor();
            editor.set_text(text);
            let screen = rainbow.ui.stage();
            screen.runtime.settle().unwrap();
            drain(&screen.redraws);
            editor.handle_input(RIGHT);
            editor.handle_input(RIGHT);
            let colored = has_rainbow(&rainbow.rows());
            assert_eq!(colored, animates, "{text:?}");
            if animates {
                rainbow.next_frame().await;
            }
            assert!(rainbow.failures.seen().is_empty());
        }
    });
}

/// Losing the keyword stops the frames; typing it again resumes the cycle where it stopped.
#[test]
fn rainbow_editor_restarted_animation_keeps_its_frame() {
    driven(async {
        let rainbow = Rainbow::new().await;
        rainbow.start_animation();
        for _ in 0..3 {
            rainbow.next_frame().await;
        }
        rainbow.type_text("");
        assert!(!has_rainbow(&rainbow.rows()));
        rainbow.type_text(&typed());
        let rows = rainbow.rows();
        assert!(shows_frame(&rows, 3), "{rows:?}");
        rainbow.next_frame().await;
        assert!(shows_frame(&rainbow.rows(), 4));
    });
}

/// The wrapped editor still answers every editor operation as the application's editor does.
#[test]
fn rainbow_editor_keeps_custom_editor_operations() {
    driven(async {
        let rainbow = Rainbow::new().await;
        let editor = rainbow.ui.editor().unwrap().editor();
        editor.set_text("draft");
        assert_eq!(editor.get_text(), "draft");
        assert_eq!(editor.insert_text_at_cursor(" text"), Some(()));
        assert_eq!(editor.get_text(), "draft text");
        assert_eq!(editor.get_expanded_text().as_deref(), Some("draft text"));
        assert_eq!(editor.add_to_history("earlier"), Some(()));
        assert_eq!(editor.set_padding_x(1.0), Some(()));
        assert_eq!(editor.set_autocomplete_max_visible(3.0), Some(()));
        assert!(editor.border_color().is_some());
        let changes = Rc::new(RefCell::new(Vec::new()));
        let seen = changes.clone();
        editor.set_on_change(Some(Rc::new(move |text: &str| {
            seen.borrow_mut().push(text.to_owned());
        })));
        editor.handle_input("!");
        assert_eq!(*changes.borrow(), ["draft text!"]);
        drop(rainbow.harness);
    });
}
