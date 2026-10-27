//! Behavioural tests for shared tool dispatch and argument validation.

use super::*;
use serde_json::json;

/// Routing returns a consumable domain or a clear no-match result.
#[test]
fn baseline_route_returns_domain_or_no_match() {
    let routed = run_route(&json!({"task":"tcp port"})).unwrap();
    let domain: serde_json::Value = serde_json::from_str(&routed).unwrap();
    assert_eq!(domain["id"], "network");
    assert_eq!(domain["primary"], "surface");
    assert_eq!(
        run_route(&json!({"task":"zzzzzz"})).unwrap(),
        "no domain matched: zzzzzz"
    );
    assert_eq!(run_route(&json!({})).unwrap_err(), "task is required");
    assert_eq!(
        run_route(&json!({"task":7})).unwrap_err(),
        "task is required"
    );
}

/// Tool runners reject missing identifiers without attempting a lookup.
#[test]
fn baseline_retained_tools_require_run_id() {
    for run in [run_report_page, run_evidence_page, run_validate] {
        assert_eq!(run(&json!({})).unwrap_err(), "run_id is required");
        assert_eq!(
            run(&json!({"run_id":false})).unwrap_err(),
            "run_id is required"
        );
    }
    assert_eq!(
        run_evidence(&json!({})).unwrap_err(),
        "run_id or agent is required"
    );
}

/// Malformed or incomplete assessment and workspace orders are rejected.
#[test]
fn baseline_assessment_and_workspace_validate_arguments() {
    assert!(
        run_assess(&json!({}))
            .unwrap_err()
            .contains("missing field `target`")
    );
    assert!(
        run_assess(&json!({"target":3}))
            .unwrap_err()
            .contains("invalid type")
    );
    assert_eq!(
        run_assess(&json!({"target":" "})).unwrap_err(),
        "assessment failed: target is required"
    );
    assert!(
        run_workspace(&json!({}))
            .unwrap_err()
            .contains("missing field `action`")
    );
    assert_eq!(
        run_workspace(&json!({"action":"invalid"})).unwrap_err(),
        "workspace failed: unknown workspace action: invalid"
    );
}

/// Disclosure dispatch reports invalid actions and missing action-specific fields.
#[test]
fn baseline_disclosure_validates_action_and_payload() {
    assert_eq!(run_disclose(&json!({})).unwrap_err(), "action is required");
    assert_eq!(
        run_disclose(&json!({"action":"invalid"})).unwrap_err(),
        "unknown disclose action: invalid"
    );
    assert_eq!(
        run_disclose(&json!({"action":"sarif"})).unwrap_err(),
        "run_id is required for sarif"
    );
    assert!(
        run_disclose(&json!({"action":"brief"}))
            .unwrap_err()
            .contains("missing field")
    );
}

/// Retained evidence resolves the newest matching agent, while an id takes priority.
#[test]
fn baseline_evidence_resolves_agent_and_prefers_explicit_id() {
    let home = std::env::temp_dir().join(format!("tools-agent-{}", std::process::id()));
    crate::evidence::with_home(&home, || {
        for (id, agent, started) in [
            ("old", "analyst", 1),
            ("new", "analyst", 2),
            ("other", "other", 3),
        ] {
            let mut ledger = crate::evidence::Ledger::new();
            ledger.record_exec("recon", "probe", id, 0);
            ledger
                .save(id, "example.test", agent, "fixture-model")
                .unwrap();
            let mut record = crate::store::RunRecord::starting(
                id.into(),
                "example.test",
                agent,
                "fixture-model",
                None,
            );
            record.started_at = started;
            crate::store::Store.upsert_run(&record).unwrap();
        }
        let newest = run_evidence(&json!({"agent":"analyst"})).unwrap();
        let newest: crate::evidence::LedgerDoc = serde_json::from_str(&newest).unwrap();
        assert_eq!(newest.run_id, "new");
        assert_eq!(newest.entries[0].output, "new");
        let explicit = run_evidence(&json!({"run_id":"old","agent":"analyst"})).unwrap();
        let explicit: crate::evidence::LedgerDoc = serde_json::from_str(&explicit).unwrap();
        assert_eq!(explicit.run_id, "old");
        assert_eq!(explicit.entries[0].output, "old");
    });
    std::fs::remove_dir_all(home).unwrap();
}

/// Missing retained documents are reported with the tool-specific lookup context.
#[test]
fn baseline_retained_tools_report_lookup_failures() {
    let home = std::env::temp_dir().join(format!("tools-missing-{}", std::process::id()));
    crate::evidence::with_home(&home, || {
        for run in [run_report_page, run_validate] {
            assert!(
                run(&json!({"run_id":"absent"}))
                    .unwrap_err()
                    .starts_with("report lookup failed:")
            );
        }
        for run in [run_evidence_page, run_evidence] {
            assert!(
                run(&json!({"run_id":"absent"}))
                    .unwrap_err()
                    .starts_with("evidence lookup failed:")
            );
        }
        assert!(
            run_disclose(&json!({"action":"sarif","run_id":"absent"}))
                .unwrap_err()
                .starts_with("report lookup failed:")
        );
    });
}
