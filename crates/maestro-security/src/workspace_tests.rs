//! Baseline behavior checks through workspace dispatch.
use super::{Order, dispatch};
use crate::evidence::{Ledger, LlmCall};
use crate::store::{Status, Store, Workspace};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicU32, Ordering};

/// Give each test its own retained state directory.
fn isolated(test: impl FnOnce()) {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let home = std::env::temp_dir().join(format!(
        "security-workspace-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    crate::evidence::with_home(&home, || {
        Store
            .save_cards("review", &[])
            .expect("existing empty deck");
        Store
            .save_creds("review", &[])
            .expect("existing empty credentials");
        Store
            .save_creds("other", &[])
            .expect("other empty credentials");
        test();
    });
    std::fs::remove_dir_all(home).expect("remove workspace state");
}

/// A named action with an optional payload.
fn order(action: &str, payload: Option<Value>) -> Order {
    Order {
        action: action.into(),
        name: Some("review".into()),
        payload,
        ..Order::default()
    }
}

/// Query the persisted deck rather than trusting the mutation response.
fn cards() -> Value {
    serde_json::to_value(
        dispatch(&order("cards:list", None))
            .expect("list cards")
            .cards,
    )
    .expect("cards json")
}

/// Baseline: new cards keep their fields and evidence-backed decisions persist.
#[test]
fn baseline_card_decisions_are_persisted() {
    isolated(|| {
        for (kind, title) in [
            ("finding", "injection"),
            ("observation", "port"),
            ("info", "scope"),
        ] {
            dispatch(&order(
                "cards:add",
                Some(json!({"kind":kind,"title":title,"body":"details"})),
            ))
            .expect("add card");
        }
        for (id, action) in [
            ("c-001", "confirm"),
            ("c-002", "refute"),
            ("c-003", "dismiss"),
        ] {
            dispatch(&order(
                "cards:update",
                Some(json!({"id":id,"do":action,"proof":"ev-0001"})),
            ))
            .expect("decide card");
        }
        let deck = cards();
        assert_eq!(deck[0]["status"], "confirmed");
        assert_eq!(deck[0]["proof"], "ev-0001");
        assert_eq!(deck[0]["kind"], "finding");
        assert_eq!(deck[0]["title"], "injection");
        assert_eq!(deck[0]["body"], "details");
        assert_eq!(deck[1]["status"], "refuted");
        assert_eq!(deck[2]["status"], "dismissed");
    });
}

/// Baseline: invalid updates cannot change the retained card.
#[test]
fn baseline_card_validation_preserves_the_deck() {
    isolated(|| {
        dispatch(&order("cards:add", Some(json!({"title":"candidate"})))).expect("add");
        let before = cards();
        for payload in [
            json!({"id":"c-001","do":"confirm"}),
            json!({"id":"c-001","do":"confirm","proof":"  "}),
            json!({"id":"c-001","do":"unknown"}),
            json!({"id":"missing","do":"refute"}),
        ] {
            assert!(dispatch(&order("cards:update", Some(payload))).is_err());
            assert_eq!(cards(), before);
        }
        assert_eq!(before[0]["kind"], "observation");
        assert_eq!(before[0]["status"], "open");
        assert_eq!(before[0]["body"], "");
    });
}

/// Baseline: credential references survive separate add/list calls and stay scoped.
#[test]
fn baseline_credentials_are_scoped_to_the_workspace() {
    isolated(|| {
        dispatch(&order("creds:add", Some(json!({
            "kind":"ssh", "target":"example.test", "secret_ref":"vault://example/ssh", "note":"test reference"
        })))).expect("add reference");
        dispatch(&order(
            "creds:add",
            Some(json!({
                "kind":"http", "target":"example.test", "secret_ref":"vault://example/http"
            })),
        ))
        .expect("second reference");
        let result = dispatch(&order("creds:list", None)).expect("list");
        assert_eq!(result.credentials.len(), 2);
        assert_eq!(result.credentials[0].id, "k-001");
        assert_eq!(result.credentials[0].kind, "ssh");
        assert_eq!(result.credentials[0].target, "example.test");
        assert_eq!(result.credentials[0].secret_ref, "vault://example/ssh");
        assert_eq!(result.credentials[0].note, "test reference");
        assert_eq!(result.credentials[1].id, "k-002");
        assert_eq!(result.credentials[1].note, "");
        let mut other = order("creds:list", None);
        other.name = Some("other".into());
        assert!(
            dispatch(&other)
                .expect("other workspace")
                .credentials
                .is_empty()
        );
    });
}

/// Baseline: required names, targets, payload shapes and fields are checked.
#[test]
fn baseline_dispatch_rejects_incomplete_actions() {
    for action in ["start", "resume", "status", "logs", "stop"] {
        assert!(
            dispatch(&Order {
                action: action.into(),
                ..Order::default()
            })
            .is_err()
        );
    }
    for action in ["start", "resume"] {
        assert!(
            dispatch(&order(action, None))
                .unwrap_err()
                .contains("target")
        );
    }
    for payload in [
        None,
        Some(json!([])),
        Some(json!({})),
        Some(json!({"title":" "})),
        Some(json!({"title":"x","kind":"unknown"})),
    ] {
        assert!(dispatch(&order("cards:add", payload)).is_err());
    }
    for payload in [
        json!({}),
        json!({"kind":"ssh"}),
        json!({"kind":"ssh","target":"x"}),
    ] {
        assert!(dispatch(&order("creds:add", Some(payload))).is_err());
    }
}

/// A retained running workspace with a known creation time and revision.
fn seed_workspace() {
    Store
        .upsert_workspace(
            0,
            &Workspace {
                name: "review".into(),
                run_id: "run-test".into(),
                target: "example.test".into(),
                agent: "maestro.security".into(),
                model: None,
                status: Status::Running,
                created_at: 1,
                updated_at: 2,
                phases_done: 0,
                findings: 0,
                revision: 0,
            },
        )
        .expect("seed workspace");
}

/// Baseline: status/list read retained state and stop persists without losing identity.
#[test]
fn baseline_stop_preserves_workspace_identity() {
    isolated(|| {
        seed_workspace();
        assert_eq!(
            dispatch(&order("list", None))
                .expect("list")
                .workspaces
                .len(),
            1
        );
        dispatch(&order("stop", None)).expect("stop");
        let workspace = dispatch(&order("status", None))
            .expect("status")
            .workspace
            .expect("workspace");
        assert!(matches!(workspace.status, Status::Stopped));
        assert_eq!(workspace.target, "example.test");
        assert_eq!(workspace.run_id, "run-test");
        assert_eq!(workspace.created_at, 1);
        assert_eq!(workspace.revision, 2);
    });
}

/// Baseline: logs select command, prompt, then a 120-character Unicode note.
#[test]
fn baseline_logs_read_the_latest_run_ledger() {
    isolated(|| {
        seed_workspace();
        assert!(
            dispatch(&order("logs", None))
                .expect("no ledger")
                .logs
                .is_empty()
        );
        let mut ledger = Ledger::new();
        ledger.record_exec("recon", "inspect target", "output", 0);
        ledger.record_llm(
            "verify",
            "reason",
            &LlmCall {
                prompt: "check evidence".into(),
                output: "answer".into(),
                model: "fixture".into(),
                tokens_out: 3,
            },
        );
        ledger.record_note("report", &"é".repeat(125));
        ledger
            .save("run-test", "example.test", "maestro.security", "fixture")
            .expect("save");
        let logs = dispatch(&order("logs", None)).expect("logs").logs;
        assert_eq!(logs.len(), 3);
        assert_eq!(logs[0].id, "ev-0001");
        assert_eq!(logs[0].phase, "recon");
        assert_eq!(logs[0].kind, "exec");
        assert_eq!(logs[0].note, "inspect target");
        assert_eq!(logs[1].note, "check evidence");
        assert_eq!(logs[2].note, "é".repeat(120));
    });
}

/// A stop response must expose the committed revision, not the pre-update value.
#[test]
fn stop_response_revision_matches_the_retained_workspace() {
    isolated(|| {
        seed_workspace();
        let stopped = dispatch(&order("stop", None))
            .expect("stop")
            .workspace
            .expect("stopped workspace");
        let retained = dispatch(&order("status", None))
            .expect("status")
            .workspace
            .expect("retained workspace");
        assert_eq!(stopped.revision, retained.revision);
        assert!(matches!(stopped.status, Status::Stopped));
    });
}
