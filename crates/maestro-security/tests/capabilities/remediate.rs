//! Baseline remediation behaviours through the public policy-gated API.

use maestro_security::remediate::{Ticket, remediate};

use super::support::{Reply, Result, case};

/// Baseline: target and findings are mandatory before any service request.
#[test]
fn requires_target_and_findings() {
    let mut params = Ticket {
        target: " \n".into(),
        ..Ticket::default()
    };
    assert_eq!(
        remediate(&params).err().as_deref(),
        Some("target is required")
    );
    params.target = "example.test".into();
    assert_eq!(
        remediate(&params).err().as_deref(),
        Some("at least one finding is required")
    );
}

/// Baseline: irreversible fixes are reported without executing either fix or recheck.
#[test]
fn policy_blocks_irreversible_fixes() -> Result {
    case(
        "remediate::policy_blocks_irreversible_fixes",
        vec![Reply::chat(
            r#"{"fixes":[{"index":0,"fix":"unsafe-fix","reversible":false,"recheck":"probe"}]}"#,
        )],
        || {
            let report = remediate(&ticket())?;
            assert_eq!(report.fixed, 0);
            assert_eq!(report.steps.len(), 1);
            let step = &report.steps[0];
            assert_eq!(step.finding, "finding a");
            assert_eq!(step.fix, "unsafe-fix");
            assert!(!step.allowed);
            assert_eq!(step.result, "not-applied");
            assert!(step.output.is_empty());
            assert!(step.recheck.is_empty());
            Ok(())
        },
    )
}

/// Baseline: valid fixes run and are rechecked; only verifier-confirmed fixes count.
#[test]
fn applies_rechecks_and_counts_verified_fixes() -> Result {
    case(
        "remediate::applies_rechecks_and_counts_verified_fixes",
        vec![
            Reply::chat(
                r#"{"fixes":[{}, {"index":9,"fix":"unused","reversible":true}, {"index":0,"fix":" ","reversible":true}, {"index":0,"fix":"fix-a","reversible":true,"recheck":"check-a"},{"index":1,"fix":"fix-b","reversible":true,"recheck":"check-b"}]}"#,
            ),
            Reply::exec("applied a", 0).command("fix-a"),
            Reply::exec("blocked a", 1).command("check-a"),
            Reply::chat(r#"{"held":true,"note":"original probe blocked"}"#),
            Reply::exec("applied b", 0).command("fix-b"),
            Reply::exec("still vulnerable", 0).command("check-b"),
            Reply::chat(r#"{"held":false,"note":"still accessible"}"#),
        ],
        || {
            let report = remediate(&ticket())?;
            assert_eq!(report.target, "example.test");
            assert_eq!(report.fixed, 1);
            assert_eq!(report.steps.len(), 2);
            let first = &report.steps[0];
            assert!(first.allowed);
            assert_eq!(first.finding, "finding a");
            assert_eq!(first.fix, "fix-a");
            assert_eq!(first.output, "applied a");
            assert_eq!(first.recheck, "blocked a");
            assert_eq!(first.result, "fixed");
            assert_eq!(first.note, "original probe blocked");
            assert_eq!(report.steps[1].finding, "finding b");
            assert_eq!(report.steps[1].result, "not-fixed");
            Ok(())
        },
    )
}

/// Baseline: invalid planning is an error, while an empty plan applies nothing.
#[test]
fn rejects_unparseable_plans() -> Result {
    case(
        "remediate::rejects_unparseable_plans",
        vec![Reply::chat("not JSON"), Reply::chat("{}")],
        || {
            assert_eq!(
                remediate(&ticket()).err().as_deref(),
                Some("remediate: model returned no parseable plan")
            );
            let report = remediate(&ticket())?;
            assert!(report.steps.is_empty());
            assert_eq!(report.fixed, 0);
            Ok(())
        },
    )
}

/// Baseline: verifier failure or malformed output never reports a verified fix.
#[test]
fn verifier_failures_and_output_limits() -> Result {
    case(
        "remediate::verifier_failures_and_output_limits",
        vec![
            Reply::chat(
                r#"{"fixes":[{"index":0,"fix":"fix-a","reversible":true},{"index":1,"fix":"fix-b","reversible":true}]}"#,
            ),
            Reply::failure("/exec"),
            Reply::failure("/exec"),
            Reply::failure("/chat/completions"),
            Reply::exec(&"é".repeat(1500), 0),
            Reply::exec(&"界".repeat(1000), 0),
            Reply::chat("not JSON"),
        ],
        || {
            let report = remediate(&ticket())?;
            assert_eq!(report.fixed, 0);
            assert_eq!(report.steps[0].result, "not-fixed");
            assert_eq!(report.steps[0].note, "verifier call failed");
            assert!(report.steps[0].output.is_empty());
            assert!(report.steps[0].recheck.is_empty());
            assert_eq!(report.steps[1].result, "not-fixed");
            assert_eq!(report.steps[1].note, "not JSON");
            assert_eq!(report.steps[1].output, "é".repeat(500));
            assert_eq!(report.steps[1].recheck, "界".repeat(333));
            Ok(())
        },
    )
}

/// A two-finding request with an explicit fixture model.
fn ticket() -> Ticket {
    Ticket {
        target: "example.test".into(),
        findings: vec!["finding a".into(), "finding b".into()],
        model: Some("fixture-model".into()),
    }
}
