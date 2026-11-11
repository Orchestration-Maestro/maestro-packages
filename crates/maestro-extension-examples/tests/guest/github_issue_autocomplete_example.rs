//! The shipped GitHub issue completion loaded into the production engine over real repositories.
//!
//! Listing and filtering the issues needs a controlled `gh` program, which the public harness
//! cannot place on the child's search path, so these tests cover how the session resolves the
//! repository and whether it installs the completion.
use super::support::{Failures, Recorder, driven, event, harness_in, load};
use maestro_extensions_wasmtime::test_support::maestro_extensions::SessionStartEvent;
use maestro_test_directory::Scratch;
use serde_json::{Value, json};
use std::process::Command;

/// The variable that names the built completion component.
const COMPONENT: &str = "MAESTRO_GITHUB_ISSUE_AUTOCOMPLETE_COMPONENT";

/// Runs Git in `root`.
fn git(root: &Scratch, args: &[&str]) {
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
}

/// A repository with the remotes `remotes` names, in the order given.
fn repository(remotes: &[(&str, &str)]) -> Scratch {
    let root = Scratch::new().unwrap();
    git(&root, &["init", "-q"]);
    for (name, url) in remotes {
        git(&root, &["remote", "add", name, url]);
    }
    root
}

/// Starts a session in `root` and returns every UI request the start made.
async fn start(root: &Scratch) -> Vec<Value> {
    let harness = harness_in(root.path().to_str().unwrap());
    let loaded = load(&harness, COMPONENT).await;
    assert!(loaded.runner.has_handlers("session_start"));
    let ui = Recorder::install(&loaded.runner);
    let reported = Failures::collect(&loaded.runner);
    let mut start: SessionStartEvent = event(json!({"type": "session_start", "reason": "startup"}));
    loaded.runner.emit(&mut start).await.unwrap();
    assert_eq!(reported.seen(), Vec::<String>::new());
    ui.take()
}

/// Whether the requests hold the notification `error`, the only request when it is there.
fn failed_with(requests: &[Value], error: &str) -> bool {
    requests.len() == 1 && requests[0][1] == json!([error, "Error"])
}

/// A directory that is not a repository, and remotes none of which name a GitHub repository,
/// are reported once and install nothing.
#[test]
fn github_issue_autocomplete_resolves_remotes_and_reports_failures() {
    driven(async {
        let not_git = Scratch::new().unwrap();
        let requests = start(&not_git).await;
        assert!(
            failed_with(
                &requests,
                "github-issue-autocomplete: cwd is not a git repository"
            ),
            "{requests:?}"
        );
        let github = "github-issue-autocomplete: cwd is not a GitHub repository";
        let unlisted = [
            vec![],
            vec![("origin", "https://other.invalid/o/r")],
            vec![
                ("one", "ssh://git@github.com/owner/repo"),
                ("two", "https://github.com/o/r/extra"),
                ("three", "https://github.com//r"),
                ("four", "git@github.com:o/"),
            ],
        ];
        for remotes in unlisted {
            let requests = start(&repository(&remotes)).await;
            assert!(failed_with(&requests, github), "{remotes:?}: {requests:?}");
        }
    });
}

/// The first remote that spells a GitHub repository installs the completion, in any of the
/// accepted spellings and wherever it sits in the list.
#[test]
fn github_issue_autocomplete_installs_for_github_remotes() {
    driven(async {
        let accepted = [
            vec![("origin", "git@github.com:owner/repo.git")],
            vec![("origin", "https://github.com/owner/repo.git")],
            vec![("origin", "http://github.com/owner/repo")],
            vec![
                ("origin", "https://other.invalid/o/r"),
                ("next", "https://github.com/owner/repo.git"),
            ],
        ];
        for remotes in accepted {
            let requests = start(&repository(&remotes)).await;
            let installed = requests
                .iter()
                .filter(|request| request[0] == "addAutocompleteProvider")
                .count();
            assert_eq!(installed, 1, "{remotes:?}: {requests:?}");
            assert!(
                !requests.iter().any(|request| request[0] == "notify"
                    && request[1][0]
                        .as_str()
                        .is_some_and(|text| text.contains("cwd is not"))),
                "{remotes:?}: {requests:?}"
            );
        }
    });
}
