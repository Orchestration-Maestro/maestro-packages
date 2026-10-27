//! Public storage regressions and baseline behaviours.
use super::{RunRecord, Store};

/// Missing workspace documents are empty collections, ready for first use.
#[test]
fn missing_decks_and_credentials_are_empty() {
    crate::test_support::isolated(|_| {
        assert!(Store.load_cards("new").expect("missing deck").is_empty());
        assert!(
            Store
                .load_creds("new")
                .expect("missing credentials")
                .is_empty()
        );
    });
}

/// Listing follows start time, including backfilled and replaced records.
#[test]
fn runs_are_listed_by_start_time_after_upserts() {
    crate::test_support::isolated(|_| {
        for (id, started_at) in [("later", 30), ("early", 10), ("middle", 20)] {
            let mut record = RunRecord::starting(id.into(), "target", "agent", "model", None);
            record.started_at = started_at;
            Store.upsert_run(&record).expect("insert");
        }
        let ids = |limit| {
            Store
                .list_runs(limit)
                .expect("list")
                .into_iter()
                .map(|run| run.run_id)
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(0), ["later", "middle", "early"]);
        assert_eq!(ids(2), ["later", "middle"]);
        let mut early = Store.get_run("early").unwrap().unwrap();
        early.started_at = 40;
        Store.upsert_run(&early).expect("replace");
        assert_eq!(ids(0), ["early", "later", "middle"]);
    });
}
/// Baseline: missing indexes and missing ids do not invent records.
#[test]
fn baseline_empty_indexes_and_unknown_ids() {
    crate::test_support::isolated(|_| {
        assert!(Store.list_runs(0).unwrap().is_empty());
        assert!(Store.get_run("absent").unwrap().is_none());
        assert!(Store.list_workspaces().unwrap().is_empty());
        assert!(Store.get_workspace("absent").unwrap().is_none());
    });
}

/// Baseline: accepted revisions persist; stale and invalid creates leave state intact.
#[test]
fn baseline_workspace_revisions_and_ordering() {
    crate::test_support::isolated(|_| {
        let mut workspace = workspace();
        assert!(Store.upsert_workspace(1, &workspace).is_err());
        assert!(Store.list_workspaces().unwrap().is_empty());
        Store.upsert_workspace(0, &workspace).unwrap();
        let saved = Store.get_workspace("first").unwrap().unwrap();
        assert_eq!(saved.revision, 1);
        assert!(Store.upsert_workspace(0, &workspace).is_err());
        workspace.phases_done = 2;
        workspace.updated_at = 30;
        Store.upsert_workspace(1, &workspace).unwrap();
        workspace.name = "second".into();
        workspace.updated_at = 20;
        Store.upsert_workspace(0, &workspace).unwrap();
        let listed = Store.list_workspaces().unwrap();
        assert_eq!(
            listed.iter().map(|w| w.name.as_str()).collect::<Vec<_>>(),
            ["first", "second"]
        );
        assert_eq!(listed[0].revision, 2);
        assert_eq!(listed[0].phases_done, 2);
    });
}

/// A fresh open card used to exercise its public lifecycle.
fn card() -> super::Card {
    super::Card {
        id: "c-001".into(),
        kind: super::CardKind::Finding,
        title: "Observed access".into(),
        body: "Restricted resource was readable".into(),
        status: super::CardStatus::Open,
        proof: None,
        created_at: 1,
    }
}

/// Baseline: each terminal disposition rejects later changes and preserves proof.
#[test]
fn baseline_card_lifecycle() {
    use super::CardStatus;
    for disposition in [
        CardStatus::Confirmed,
        CardStatus::Refuted,
        CardStatus::Dismissed,
    ] {
        let mut card = card();
        assert!(card.is_valid());
        match disposition {
            CardStatus::Confirmed => assert!(card.confirm("observed response")),
            CardStatus::Refuted => assert!(card.refute()),
            CardStatus::Dismissed => assert!(card.dismiss()),
            CardStatus::Open => unreachable!(),
        }
        assert_eq!(card.status, disposition);
        let proof = card.proof.clone();
        assert!(!card.confirm("replacement"));
        assert!(!card.refute());
        assert!(!card.dismiss());
        assert_eq!(card.status, disposition);
        assert_eq!(card.proof, proof);
        assert!(card.is_valid());
    }
    let mut invalid = card();
    invalid.status = CardStatus::Confirmed;
    assert!(!invalid.is_valid());
}

/// Baseline: cards and secret references survive reload and stay workspace-scoped.
#[test]
fn baseline_workspace_documents_round_trip() {
    crate::test_support::isolated(|_| {
        let mut card = card();
        card.confirm("proof retained");
        Store.save_cards("one", &[card]).unwrap();
        let loaded = Store.load_cards("one").unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "c-001");
        assert_eq!(loaded[0].proof.as_deref(), Some("proof retained"));
        assert_eq!(loaded[0].status, super::CardStatus::Confirmed);
        let credential = super::Credential {
            id: "k-001".into(),
            kind: "bearer".into(),
            target: "example.test".into(),
            secret_ref: "assessment-token".into(),
            note: "read-only".into(),
            created_at: 2,
        };
        Store.save_creds("one", &[credential]).unwrap();
        let loaded = Store.load_creds("one").unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].secret_ref, "assessment-token");
        assert_eq!(loaded[0].target, "example.test");
        Store.save_cards("two", &[]).unwrap();
        Store.save_creds("two", &[]).unwrap();
        assert!(Store.load_cards("two").unwrap().is_empty());
        assert!(Store.load_creds("two").unwrap().is_empty());
        assert_eq!(Store.load_cards("one").unwrap().len(), 1);
        assert_eq!(Store.load_creds("one").unwrap().len(), 1);
    });
}

/// Baseline: corrupt persisted state reports an error instead of silently resetting.
#[test]
fn baseline_malformed_documents_are_errors() {
    crate::test_support::isolated(|home| {
        for name in [
            "runs.jsonl",
            "workspaces.json",
            "cards-one.json",
            "creds-one.json",
        ] {
            std::fs::write(home.join(name), "{broken").unwrap();
        }
        assert!(Store.list_runs(0).is_err());
        assert!(Store.get_run("one").is_err());
        assert!(Store.list_workspaces().is_err());
        assert!(
            Store
                .upsert_workspace(
                    0,
                    &super::Workspace {
                        name: "one".into(),
                        run_id: String::new(),
                        target: String::new(),
                        agent: String::new(),
                        model: None,
                        status: super::Status::Running,
                        created_at: 0,
                        updated_at: 0,
                        phases_done: 0,
                        findings: 0,
                        revision: 0,
                    }
                )
                .is_err()
        );
        assert!(Store.load_cards("one").is_err());
        assert!(Store.load_creds("one").is_err());
        let record = RunRecord::starting("one".into(), "t", "a", "m", None);
        assert!(Store.upsert_run(&record).is_err());
    });
}

/// Baseline: settling completes counts and model, or retains a failure cause.
#[test]
fn baseline_run_outcomes() {
    let before = super::now_secs();
    let mut record = RunRecord::starting("run".into(), "t", "a", "pending", Some("workspace"));
    assert_eq!(record.status, super::Status::Running);
    assert!(record.finished_at.is_none());
    assert_eq!(record.workspace.as_deref(), Some("workspace"));
    assert!(record.started_at >= before);
    let mut report = crate::test_support::assessment();
    report.refuted.push(crate::report::Refuted {
        class: "access".into(),
        hypothesis: "public resource".into(),
        evidence_id: "ev-0001".into(),
    });
    record.settle(Ok(&report));
    assert_eq!(record.status, super::Status::Completed);
    assert_eq!(record.model, report.model);
    assert_eq!(record.summary, report.summary);
    assert_eq!(record.refuted, 1);
    assert_eq!(record.findings, 1);
    assert!(record.finished_at.unwrap() >= record.started_at);
    record.settle(Err("runner disconnected"));
    assert_eq!(record.status, super::Status::Failed);
    assert_eq!(record.summary, "runner disconnected");
}

/// A new workspace for accepted and rejected persistence updates.
fn workspace() -> super::Workspace {
    super::Workspace {
        name: "first".into(),
        run_id: "run-one".into(),
        target: "target".into(),
        agent: "security".into(),
        model: Some("model".into()),
        status: super::Status::Running,
        created_at: 1,
        updated_at: 10,
        phases_done: 0,
        findings: 0,
        revision: 0,
    }
}

/// A run index that exists but cannot be read must not look empty.
#[test]
fn unreadable_run_index_is_an_error() {
    crate::test_support::isolated(|home| {
        std::fs::create_dir(home.join("runs.jsonl")).unwrap();
        assert!(Store.list_runs(0).is_err());
        assert!(Store.get_run("absent").is_err());
    });
}

/// An unreadable workspace table must not disappear from the run board.
#[test]
fn unreadable_workspace_listing_is_an_error() {
    crate::test_support::isolated(|home| {
        std::fs::create_dir(home.join("workspaces.json")).unwrap();
        assert!(Store.list_workspaces().is_err());
        assert!(Store.get_workspace("absent").is_err());
    });
}

/// Updating must not overwrite a retained table that cannot be decoded as text.
#[test]
fn unreadable_workspace_update_preserves_retained_bytes() {
    crate::test_support::isolated(|home| {
        let path = home.join("workspaces.json");
        let retained = [0xff, 0xfe];
        std::fs::write(&path, retained).unwrap();
        assert!(Store.upsert_workspace(0, &workspace()).is_err());
        assert_eq!(std::fs::read(path).unwrap(), retained);
    });
}
