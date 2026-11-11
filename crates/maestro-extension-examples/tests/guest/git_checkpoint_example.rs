//! The shipped checkpoint hooks loaded into the production engine over disposable repositories.
use super::support::{Failures, Recorder, driven, event, harness_in, load};
use maestro_extensions_wasmtime::test_support::{
    Harness, Loaded,
    maestro_extensions::{AgentEndEvent, SessionBeforeForkEvent, TurnStartEvent},
};
use maestro_models::UserContent;
use maestro_session::{Presence, session_manager::CompactionOptions};
use maestro_test_directory::Scratch;
use serde_json::{Value, json};
use std::{process::Command, rc::Rc};

/// The variable that names the built checkpoint component.
const COMPONENT: &str = "MAESTRO_GIT_CHECKPOINT_COMPONENT";
/// The selector choice that applies the checkpoint.
const YES: &str = "Yes, restore code to that point";

/// Runs Git in the disposable repository and returns its output.
fn git(root: &Scratch, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(root.path())
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// One committed repository, its loaded checkpoint component and the UI it shows.
struct Checkpointing {
    /// The repository, deleted when the test ends.
    root: Scratch,
    /// The engine the component lives in.
    harness: Harness,
    /// The runner over the component.
    loaded: Loaded,
    /// The selector and notification requests.
    ui: Rc<Recorder>,
    /// The callback failures the runner reported.
    failures: Failures,
}

impl Checkpointing {
    /// Initializes a repository with one committed file and loads the component over it.
    async fn new() -> Self {
        let root = Scratch::new().unwrap();
        git(&root, &["init", "-q"]);
        git(
            &root,
            &["config", "user.email", "checkpoint@example.invalid"],
        );
        git(&root, &["config", "user.name", "Checkpoint test"]);
        std::fs::write(root.path().join("tracked.txt"), "committed\n").unwrap();
        git(&root, &["add", "tracked.txt"]);
        git(&root, &["commit", "-qm", "initial"]);
        Self::over(root).await
    }

    /// Loads the component with a session working in `root`.
    async fn over(root: Scratch) -> Self {
        let harness = harness_in(root.path().to_str().unwrap());
        let loaded = load(&harness, COMPONENT).await;
        let ui = Recorder::install(&loaded.runner);
        let failures = Failures::collect(&loaded.runner);
        Self {
            root,
            harness,
            loaded,
            ui,
            failures,
        }
    }

    /// Writes the tracked file without touching the index.
    fn write(&self, text: &str) {
        std::fs::write(self.root.path().join("tracked.txt"), text).unwrap();
    }

    /// Reads the working-tree contents.
    fn text(&self) -> String {
        std::fs::read_to_string(self.root.path().join("tracked.txt")).unwrap()
    }

    /// Lets the registered tool-result hook track the session's current leaf.
    async fn tool_result(&self) {
        let document = json!({
            "toolCallId": "read", "toolName": "read", "input": {}, "content": [], "isError": false
        });
        let event = serde_json::from_value(document).unwrap();
        self.loaded.runner.emit_tool_result(&event).await.unwrap();
    }

    /// Captures a checkpoint through the registered turn-start hook.
    async fn turn(&self) {
        let mut turn: TurnStartEvent =
            event(json!({"type": "turn_start", "turnIndex": 0, "timestamp": 0}));
        self.loaded.runner.emit(&mut turn).await.unwrap();
    }

    /// Requests a fork at `id` answering the selector with `choice`; returns the UI requests.
    async fn fork(&self, id: &str, choice: Option<&str>) -> Vec<Value> {
        self.ui.answer(json!(choice));
        let mut fork: SessionBeforeForkEvent =
            event(json!({"type": "session_before_fork", "entryId": id, "position": "at"}));
        self.loaded.runner.emit(&mut fork).await.unwrap();
        self.ui.take()
    }

    /// Appends an entry with the session writer and returns its identity.
    fn append(&self, write: impl FnOnce(&mut maestro_session::SessionManager) -> String) -> String {
        write(&mut self.harness.session_manager.borrow_mut())
    }
}

/// Appends one entry to a session and returns its identity.
type Append = fn(&mut maestro_session::SessionManager) -> String;

/// One session entry of each of the nine kinds, each appended as the current leaf.
fn entry_kinds() -> Vec<(&'static str, Append)> {
    vec![
        ("message", |s| {
            s.append_message(json!({"role": "user", "content": "q", "timestamp": 0}))
                .unwrap()
        }),
        ("thinking_level_change", |s| {
            s.append_thinking_level_change("high").unwrap()
        }),
        ("model_change", |s| {
            s.append_model_change("provider", "model").unwrap()
        }),
        ("compaction", |s| {
            let first = s.append_session_info("first").unwrap();
            s.append_compaction("summary", &first, 1.0, CompactionOptions::default())
                .unwrap()
        }),
        ("branch_summary", |s| {
            let from = s.append_session_info("branch").unwrap();
            s.branch_with_summary(Some(&from), "summary", Presence::Missing, Presence::Missing)
                .unwrap()
        }),
        ("custom", |s| {
            s.append_custom_entry("state", Presence::Missing).unwrap()
        }),
        ("custom_message", |s| {
            s.append_custom_message_entry(
                "note",
                UserContent::Text("text".into()),
                true,
                Presence::Missing,
            )
            .unwrap()
        }),
        ("label", |s| {
            let target = s.append_session_info("target").unwrap();
            s.append_label_change(&target, Presence::Present("mark"))
                .unwrap()
        }),
        ("session_info", |s| s.append_session_info("name").unwrap()),
    ]
}

/// Every kind of leaf entry keeps its own checkpoint; empty Git output keeps the previous one and
/// the end of the agent run forgets them all.
#[test]
fn git_checkpoint_factory_preserves_commands_whitespace_and_entry_kinds() {
    driven(async {
        let checkpointing = Checkpointing::new().await;
        let mut last = String::new();
        for (kind, make) in entry_kinds() {
            let id = checkpointing.append(make);
            last.clone_from(&id);
            checkpointing.tool_result().await;
            checkpointing.write(&format!("{kind} checkpoint\n"));
            checkpointing.turn().await;
            git(&checkpointing.root, &["checkout", "--", "tracked.txt"]);
            // A clean tree makes Git print nothing, which must not replace the checkpoint.
            checkpointing.turn().await;
            let shown = checkpointing.fork(&id, Some(YES)).await;
            assert_eq!(shown.len(), 2, "{kind}: {shown:?}");
            assert_eq!(shown[1][1], json!(["Code restored to checkpoint", "Info"]));
            assert_eq!(
                checkpointing.text(),
                format!("{kind} checkpoint\n"),
                "{kind}"
            );
            git(&checkpointing.root, &["checkout", "--", "tracked.txt"]);
        }
        let mut ended: AgentEndEvent = event(json!({"type": "agent_end", "messages": []}));
        checkpointing.loaded.runner.emit(&mut ended).await.unwrap();
        assert!(checkpointing.fork(&last, Some(YES)).await.is_empty());
        assert!(checkpointing.failures.seen().is_empty());
    });
}

/// A checkpoint that cannot be applied reports Git's refusal and never announces a restore.
#[test]
fn git_checkpoint_factory_rejects_failed_restores() {
    driven(async {
        let checkpointing = Checkpointing::new().await;
        let id =
            checkpointing.append(|s| s.append_custom_entry("state", Presence::Missing).unwrap());
        checkpointing.tool_result().await;
        checkpointing.write("checkpointed\n");
        checkpointing.turn().await;
        checkpointing.write("edited since\n");
        let shown = checkpointing.fork(&id, Some(YES)).await;
        assert_eq!(shown.len(), 1, "no success notification: {shown:?}");
        assert_eq!(shown[0][0], "select");
        let failures = checkpointing.failures.seen();
        assert_eq!(failures.len(), 1, "{failures:?}");
        let report = failures[0]
            .strip_prefix("Could not restore checkpoint (exit code ")
            .unwrap_or_else(|| panic!("unexpected report: {failures:?}"));
        let (code, explanation) = report
            .split_once(", killed: false): ")
            .unwrap_or_else(|| panic!("unexpected report: {failures:?}"));
        assert_ne!(code.parse::<i32>().unwrap(), 0, "{failures:?}");
        assert!(!explanation.trim().is_empty(), "{failures:?}");
        assert_eq!(checkpointing.text(), "edited since\n");
    });
}

/// A refused selector or notification ends the callback with the host's failure.
#[test]
fn git_checkpoint_factory_propagates_host_failures() {
    driven(async {
        let checkpointing = Checkpointing::new().await;
        let id =
            checkpointing.append(|s| s.append_custom_entry("state", Presence::Missing).unwrap());
        checkpointing.tool_result().await;
        checkpointing.write("checkpointed\n");
        checkpointing.turn().await;
        git(&checkpointing.root, &["checkout", "--", "tracked.txt"]);
        checkpointing.ui.reject("select", "selector unavailable");
        assert_eq!(checkpointing.fork(&id, Some(YES)).await.len(), 1);
        assert_eq!(checkpointing.text(), "committed\n", "nothing was applied");
        checkpointing
            .ui
            .reject("notify", "notification unavailable");
        assert_eq!(checkpointing.fork(&id, Some(YES)).await.len(), 2);
        assert_eq!(
            checkpointing.text(),
            "checkpointed\n",
            "applied before the notice failed"
        );
        assert_eq!(
            checkpointing.failures.seen(),
            ["selector unavailable", "notification unavailable"]
        );
    });
}
