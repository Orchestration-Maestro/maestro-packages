//! The shipped dirty-repository guard loaded into the production engine, over a real repository.
use super::lane_b::{Failures, Instance, Recorder, driven, emit};
use maestro_extensions_wasmtime::test_support::maestro_extensions::{
    Presence, SessionBeforeForkEvent, SessionBeforeSwitchEvent,
};
use serde_json::{Value, json};
use std::{path::Path, process::Command};

/// The variable that names the built guard component.
const COMPONENT: &str = "MAESTRO_DIRTY_REPO_GUARD_COMPONENT";

/// The selector choices, affirmative first.
const CHOICES: [&str; 2] = ["Continue", "Cancel and commit first"];

/// A guarded change: the event it raises and the action its prompt names.
struct Change {
    /// Event body delivered to the registered callback.
    event: Value,
    /// Whether it is a fork rather than a session switch.
    fork: bool,
    /// The action the prompt names.
    action: &'static str,
}

/// The three guarded session changes.
fn changes() -> [Change; 3] {
    [
        Change {
            event: json!({"reason": "new"}),
            fork: false,
            action: "Start a new session",
        },
        Change {
            event: json!({"reason": "resume", "targetSessionFile": "other.jsonl"}),
            fork: false,
            action: "Switch sessions",
        },
        Change {
            event: json!({"entryId": "entry", "position": "at"}),
            fork: true,
            action: "Fork",
        },
    ]
}

/// The selector request for `changed` files before `action`.
fn select(action: &str, changed: usize) -> Value {
    json!([
        "select",
        [
            format!("{action} with {changed} uncommitted file(s)?"),
            CHOICES,
            null,
            null
        ]
    ])
}

/// The warning sent after a refusal.
fn warning() -> Value {
    json!([
        "notify",
        ["Cancelled. Commit the changes, then try again.", "Warning"]
    ])
}

/// Runs git in `path` and checks that it succeeded.
fn git(path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(path)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A repository with `files` untracked files and its own status configuration.
fn repository(files: &[&str]) -> maestro_test_directory::Scratch {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    git(scratch.path(), &["init", "--quiet"]);
    git(
        scratch.path(),
        &["config", "status.showUntrackedFiles", "all"],
    );
    for file in files {
        std::fs::write(scratch.path().join(file), "text").unwrap();
    }
    scratch
}

/// What one change produced: whether it was cancelled, the UI requests and the callback failures.
struct Outcome {
    /// Whether a handler vetoed the change.
    cancelled: bool,
    /// The requests the UI recorded.
    requests: Vec<Value>,
    /// The failures the handlers reported.
    failures: Vec<String>,
}

/// Delivers `change` to a guard started in `directory`.
///
/// `ui` is whether the application has a UI; `choice` is the selector's answer, and `reject` the
/// method the UI refuses.
async fn deliver(
    directory: &Path,
    change: &Change,
    ui: bool,
    choice: Option<&str>,
    reject: Option<&str>,
) -> Outcome {
    let instance = Instance::in_directory(COMPONENT, directory.to_str().unwrap()).await;
    let failures = Failures::collect(instance.runner());
    let recorder = Recorder::install(instance.runner());
    if !ui {
        instance.runner().set_ui_context(None);
    }
    if let Some(choice) = choice {
        recorder.answer(json!(choice));
    }
    if let Some(method) = reject {
        recorder.reject(method, "UI refused");
    }
    let cancelled = if change.fork {
        matches!(
            emit::<SessionBeforeForkEvent>(instance.runner(), &change.event).await,
            Some(result) if matches!(result.cancel, Presence::Present(true))
        )
    } else {
        matches!(
            emit::<SessionBeforeSwitchEvent>(instance.runner(), &change.event).await,
            Some(result) if matches!(result.cancel, Presence::Present(true))
        )
    };
    Outcome {
        cancelled,
        requests: recorder.take(),
        failures: failures.take(),
    }
}

/// A clean tree, an empty directory with no repository and a missing directory let every
/// change go ahead without asking anything.
#[test]
fn dirty_repo_guard_session_changes() {
    driven(async {
        let dirty = repository(&["tracked.txt", "new file.txt"]);
        let one = repository(&["only.txt"]);
        let clean = repository(&[]);
        let no_repository = maestro_test_directory::Scratch::new().unwrap();
        for change in changes() {
            for directory in [clean.path(), no_repository.path()] {
                let outcome = deliver(directory, &change, true, None, None).await;
                assert!(!outcome.cancelled, "{}", change.action);
                assert!(outcome.requests.is_empty(), "{}", change.action);
                assert!(outcome.failures.is_empty(), "{}", change.action);
            }
            let headless = deliver(dirty.path(), &change, false, None, None).await;
            assert!(
                headless.cancelled,
                "{}: no UI refuses a dirty tree",
                change.action
            );
            assert!(headless.requests.is_empty());
            let go_ahead = deliver(dirty.path(), &change, true, Some(CHOICES[0]), None).await;
            assert!(!go_ahead.cancelled);
            assert_eq!(go_ahead.requests, [select(change.action, 2)]);
            let single = deliver(one.path(), &change, true, Some(CHOICES[0]), None).await;
            assert_eq!(single.requests, [select(change.action, 1)]);
            for choice in [Some(CHOICES[1]), None, Some("continue")] {
                let refused = deliver(dirty.path(), &change, true, choice, None).await;
                assert!(refused.cancelled, "{} {choice:?}", change.action);
                assert_eq!(refused.requests, [select(change.action, 2), warning()]);
            }
            let rejected_select = deliver(
                dirty.path(),
                &change,
                true,
                Some(CHOICES[1]),
                Some("select"),
            )
            .await;
            assert_eq!(rejected_select.failures, ["UI refused"]);
            assert_eq!(rejected_select.requests, [select(change.action, 2)]);
            let rejected_notify = deliver(
                dirty.path(),
                &change,
                true,
                Some(CHOICES[1]),
                Some("notify"),
            )
            .await;
            assert_eq!(rejected_notify.failures, ["UI refused"]);
            assert_eq!(
                rejected_notify.requests,
                [select(change.action, 2), warning()]
            );
        }
    });
}

/// A directory with no repository, because it cannot be entered, lets every change go ahead
/// without any question.
#[test]
fn dirty_repo_guard_lets_changes_go_ahead_when_the_status_run_fails() {
    driven(async {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let missing = scratch.path().join("missing");
        for change in changes() {
            let outcome = deliver(&missing, &change, true, None, None).await;
            assert!(outcome.failures.is_empty(), "{}", change.action);
            assert!(!outcome.cancelled);
            assert!(outcome.requests.is_empty());
        }
    });
}
