//! Baseline audit behaviours with a runner that never executes host commands.

use crate::support;
use maestro_security::audit::{Scope, audit};
use serde_json::Value;

/// Return a mix of clean output, adverse evidence and a runner error.
fn mixed(path: &str, body: &Value) -> (u16, String) {
    match path {
        "/models" => support::models(),
        "/chat/completions" => support::completion("Two checks passed; inspect the failed check."),
        "/exec" => {
            let command = body["command"].as_str().unwrap_or("");
            if command.starts_with("find ") {
                support::execution("", 0)
            } else if command.starts_with("awk ") {
                support::execution(&"é".repeat(301), 0)
            } else if command.starts_with("grep ") {
                (403, "界".repeat(301))
            } else if command.starts_with("file ") {
                support::execution("0\n", 0)
            } else {
                support::execution("2\n", 0)
            }
        }
        _ => (404, String::new()),
    }
}

/// Baseline: validators retain bounded Unicode evidence and tally pass/fail/warn.
#[test]
fn mixed_outcomes_retain_evidence_and_counts() -> Result<(), Box<dyn std::error::Error>> {
    if !support::isolated("audit::mixed_outcomes_retain_evidence_and_counts", mixed)? {
        return Ok(());
    }
    let report = audit(&Scope {
        target: "fixture-host".into(),
        model: Some("primary".into()),
    })?;
    assert_eq!(report.schema, "maestro-security/audit/1");
    assert_eq!(report.target, "fixture-host");
    assert_eq!((report.passed, report.failed, report.warned), (2, 1, 2));
    assert_eq!(report.checks.len(), 5);
    let statuses: Vec<_> = report
        .checks
        .iter()
        .map(|check| check.status.as_str())
        .collect();
    assert_eq!(statuses, ["pass", "warn", "fail", "pass", "warn"]);
    assert!(report.checks.iter().all(|check| !check.name.is_empty()));
    assert_eq!(report.checks[1].evidence, "é".repeat(250));
    assert!(report.checks[2].evidence.len() <= 500);
    assert!(report.checks[2].evidence.contains("403"));
    assert!(report.checks[2].evidence.contains(&"界".repeat(100)));
    assert_eq!(report.checks[4].evidence, "2\n");
    assert_eq!(
        report.summary,
        "Two checks passed; inspect the failed check."
    );
    Ok(())
}

/// Allow validators but deny the model summary.
fn summary_denied(path: &str, _: &Value) -> (u16, String) {
    match path {
        "/models" => support::models(),
        "/exec" => support::execution("", 0),
        _ => (403, "summary denied".into()),
    }
}

/// Baseline: a summary failure is returned rather than hidden in a report.
#[test]
fn summary_failure_is_returned() -> Result<(), Box<dyn std::error::Error>> {
    if !support::isolated("audit::summary_failure_is_returned", summary_denied)? {
        return Ok(());
    }
    let error = audit(&Scope {
        target: "fixture-host".into(),
        model: None,
    })
    .unwrap_err();
    assert!(error.contains("403"));
    assert!(error.contains("summary denied"));
    Ok(())
}

/// Baseline: empty scope is rejected before reaching external services.
#[test]
fn whitespace_target_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    if !support::isolated("audit::whitespace_target_is_rejected", summary_denied)? {
        return Ok(());
    }
    let error = audit(&Scope {
        target: " \n\t".into(),
        model: None,
    })
    .unwrap_err();
    assert_eq!(error, "target is required");
    Ok(())
}
