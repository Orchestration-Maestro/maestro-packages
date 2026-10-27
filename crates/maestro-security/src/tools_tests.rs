//! Behavioural tests for shared tool dispatch and argument validation.

use serde_json::json;
use super::*;

/// Routing returns a consumable domain or a clear no-match result.
#[test]
fn baseline_route_returns_domain_or_no_match() {
    let routed = run_route(&json!({"task":"tcp port"})).unwrap();
    let domain: serde_json::Value = serde_json::from_str(&routed).unwrap();
    assert_eq!(domain["id"], "network");
    assert_eq!(domain["primary"], "surface");
    assert_eq!(run_route(&json!({"task":"zzzzzz"})).unwrap(), "no domain matched: zzzzzz");
    assert_eq!(run_route(&json!({})).unwrap_err(), "task is required");
    assert_eq!(run_route(&json!({"task":7})).unwrap_err(), "task is required");
}

/// Tool runners reject missing identifiers without attempting a lookup.
#[test]
fn baseline_retained_tools_require_run_id() {
    for run in [run_report_page, run_evidence_page, run_validate] {
        assert_eq!(run(&json!({})).unwrap_err(), "run_id is required");
        assert_eq!(run(&json!({"run_id":false})).unwrap_err(), "run_id is required");
    }
    assert_eq!(run_evidence(&json!({})).unwrap_err(), "run_id or agent is required");
}

/// Malformed or incomplete assessment and workspace orders are rejected.
#[test]
fn baseline_assessment_and_workspace_validate_arguments() {
    assert!(run_assess(&json!({})).unwrap_err().contains("missing field `target`"));
    assert!(run_assess(&json!({"target":3})).unwrap_err().contains("invalid type"));
    assert_eq!(run_assess(&json!({"target":" "})).unwrap_err(), "assessment failed: target is required");
    assert!(run_workspace(&json!({})).unwrap_err().contains("missing field `action`"));
    assert_eq!(run_workspace(&json!({"action":"invalid"})).unwrap_err(), "workspace failed: unknown workspace action: invalid");
}

/// Disclosure dispatch reports invalid actions and missing action-specific fields.
#[test]
fn baseline_disclosure_validates_action_and_payload() {
    assert_eq!(run_disclose(&json!({})).unwrap_err(), "action is required");
    assert_eq!(run_disclose(&json!({"action":"invalid"})).unwrap_err(), "unknown disclose action: invalid");
    assert_eq!(run_disclose(&json!({"action":"sarif"})).unwrap_err(), "run_id is required for sarif");
    assert!(run_disclose(&json!({"action":"brief"})).unwrap_err().contains("missing field"));
}
