//! The shipped file trigger loaded into the production engine over the real file system.
//!
//! The component watches one fixed path, so each test owns it exclusively: the nextest group
//! runs these tests one at a time and the lock keeps other file-trigger tests out.
#![cfg(target_os = "linux")]
use super::lane_a::{Ui, event, failures, load_native, local_native};
use super::support::harness;
use maestro_extensions_wasmtime::test_support::maestro_extensions::SessionStartEvent;
use serde_json::{Value, json};
use std::{cell::Cell, io::Write, path::Path, rc::Rc};
use tokio::sync::mpsc;

/// The variable that names the built file-trigger component.
const COMPONENT: &str = "MAESTRO_FILE_TRIGGER_COMPONENT";
/// The fixed path the component watches.
const TRIGGER: &str = "/tmp/agent-trigger.txt";

/// Exclusive ownership of the trigger file: it never touches a file that already exists.
struct Trigger {
    /// Keeps other file-trigger tests out while this one runs.
    _lock: maestro_lock::WriteLock,
    /// The exclusively created file.
    file: std::fs::File,
}

impl Trigger {
    /// Creates the trigger file, refusing to reuse one that exists.
    fn new() -> Self {
        let lock = maestro_lock::acquire_write(Path::new(TRIGGER), true)
            .expect("the host holds one file-trigger test at a time");
        let file =
            std::fs::File::create_new(TRIGGER).expect("the trigger file is exclusive to this test");
        Self { _lock: lock, file }
    }

    /// Replaces the file's contents.
    fn write(&mut self, bytes: &[u8]) {
        use std::io::Seek;
        self.file.set_len(0).unwrap();
        self.file.rewind().unwrap();
        self.file.write_all(bytes).unwrap();
        self.file.flush().unwrap();
    }
}

impl Drop for Trigger {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(TRIGGER);
    }
}

/// A loaded component, the messages it sent and whether the host accepts them.
struct Watching {
    /// The engine the component lives in.
    harness: maestro_extensions_wasmtime::test_support::Harness,
    /// The runner over the component.
    runner: Rc<
        maestro_extensions_wasmtime::test_support::maestro_extensions::ExtensionRunner<
            maestro_extensions_wasmtime::test_support::maestro_extensions::Local,
        >,
    >,
    /// The notifications the component showed.
    ui: Rc<Ui>,
    /// Every message the component sent, with its options, or a refusal marker.
    sent: mpsc::UnboundedReceiver<Value>,
    /// Whether the host refuses the next messages.
    refusing: Rc<Cell<bool>>,
}

impl Watching {
    /// Loads the component and routes its messages to the receiver.
    async fn new(local: &Rc<tokio::task::LocalSet>) -> Self {
        let harness = harness();
        let loaded = load_native(&harness, COMPONENT, local).await;
        let (sender, sent) = mpsc::unbounded_channel();
        let refusing = Rc::new(Cell::new(false));
        let refused = refusing.clone();
        harness.runtime.actions.write().unwrap().send_message = Rc::new(move |message, options| {
            let document = json!([message, options]);
            if refused.get() {
                sender.send(json!({"refused": document})).unwrap();
                return Err("delivery refused".into());
            }
            sender.send(document).unwrap();
            Ok(())
        });
        let ui = Ui::install(&loaded.runner);
        Self {
            harness,
            runner: loaded.runner,
            ui,
            sent,
            refusing,
        }
    }

    /// Delivers a session start, which replaces any earlier watch.
    async fn start(&self) {
        let mut start: SessionStartEvent =
            event(json!({"type": "session_start", "reason": "startup"}));
        self.runner.emit(&mut start).await.unwrap();
    }

    /// Waits for the next message the component sent.
    async fn next(&mut self) -> Value {
        self.sent.recv().await.unwrap()
    }

    /// Waits for the next message the host accepted, passing over refused attempts and further
    /// deliveries of `repeated`, which one change of the file may announce more than once.
    async fn next_accepted(&mut self, repeated: &Value) -> Value {
        loop {
            let sent = self.next().await;
            if sent.get("refused").is_none() && sent != *repeated {
                return sent;
            }
        }
    }
}

/// The message document the component sends for `content`.
fn message(content: &str) -> Value {
    json!([
        {"customType": "file-trigger", "content": format!("External trigger: {content}"), "display": true},
        {"triggerTurn": true}
    ])
}

/// Nonempty contents are sent trimmed and cleared; empty-looking contents stay silent.
#[test]
fn file_trigger_delivers_nonempty_contents_and_releases_watches() {
    local_native(|local| async move {
        let mut trigger = Trigger::new();
        let mut watching = Watching::new(&local).await;
        let reported = failures(&watching.runner);
        watching.start().await;
        assert_eq!(*reported.borrow(), Vec::<String>::new());
        assert_eq!(
            watching.ui.take()[0]["operands"],
            json!(["Watching /tmp/agent-trigger.txt", "Info"])
        );
        trigger.write(" \u{feff}\nRun the tests Ω\n\u{feff} ".as_bytes());
        assert_eq!(watching.next().await, message("Run the tests Ω"));
        assert_eq!(std::fs::read_to_string(TRIGGER).unwrap(), "");
        for text in ["", " \r\n\t", "\u{2003}\u{a0}", "\u{feff}", " \u{feff}\t"] {
            trigger.write(text.as_bytes());
        }
        // The next message is the marker alone: no empty-looking text produced one before it.
        watching.ui.take();
        watching.runner.set_ui_context(None);
        watching.start().await;
        assert!(
            watching.ui.take().is_empty(),
            "no notification without a UI"
        );
        trigger.write("\u{85}second\u{85}".as_bytes());
        assert_eq!(watching.next().await, message("\u{85}second\u{85}"));
        assert!(reported.borrow().is_empty());
        drop(watching.harness);
    });
}

/// A refused delivery keeps the text for a retry, and invalid UTF-8 is replaced, not dropped.
#[test]
fn file_trigger_ignores_transient_errors_without_clearing_rejected_text() {
    local_native(|local| async move {
        let mut trigger = Trigger::new();
        let mut watching = Watching::new(&local).await;
        watching.start().await;
        watching.refusing.set(true);
        trigger.write(b"retry me");
        assert_eq!(
            watching.next().await,
            json!({"refused": message("retry me")})
        );
        assert_eq!(std::fs::read_to_string(TRIGGER).unwrap(), "retry me");
        watching.refusing.set(false);
        trigger.write(b"retry me");
        let retried = watching.next_accepted(&Value::Null).await;
        assert_eq!(retried, message("retry me"));
        assert_eq!(std::fs::read_to_string(TRIGGER).unwrap(), "");
        trigger.write(b"invalid \xff UTF-8");
        let decoded = watching.next_accepted(&retried).await;
        assert_eq!(decoded, message("invalid \u{fffd} UTF-8"));
    });
}
