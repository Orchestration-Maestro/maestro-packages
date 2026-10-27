//! Live end-to-end assessment against the local bridge and runner.
//!
//! Runs whenever the configured bridge answers `/models` (defaults: the
//! local model router and its `qwythos` model), skipping when it does not.
//! The model is whatever the bridge serves at run time (or `ASSESS_MODEL`
//! when set), so this test follows a bridge model swap without code changes.

use maestro_security::{AssessParams, Client, Config, Finding, assess, evidence_for};

/// Fail the test on an error, otherwise return the value.
fn ok_or_fail<T, E>(result: Result<T, E>, what: &str) -> T
where
    E: std::fmt::Display,
{
    match result {
        Ok(value) => value,
        Err(error) => {
            let message = format!("{what}: {error}");
            assert!(message.is_empty(), "{message}");
            // Unreachable: the assertion above fails the test first.
            loop {
                std::thread::park();
            }
        }
    }
}

/// Assert the phase sequence invariants: recon opens, report closes, and
/// every required middle phase is present.
fn assert_phases(names: &[&str]) {
    assert_eq!(
        names.first().copied(),
        Some("recon"),
        "recon opens: {names:?}"
    );
    assert_eq!(
        names.last().copied(),
        Some("report"),
        "report closes: {names:?}"
    );
    for required in ["target-model", "hypotheses", "verify", "threat"] {
        assert!(names.contains(&required), "{required} missing: {names:?}");
    }
}

/// Assert one finding carries an F-id, a real evidence level and a command.
fn assert_finding(finding: &Finding) {
    assert!(finding.id.starts_with("F-"), "finding id: {}", finding.id);
    assert!(
        matches!(finding.evidence_level.as_str(), "CONFIRMED" | "LIKELY"),
        "evidence level of {}: {}",
        finding.id,
        finding.evidence_level
    );
    assert!(!finding.evidence.command.trim().is_empty());
    assert!(!finding.evidence.id.trim().is_empty());
}

/// Run one live assessment and assert the report invariants.
#[test]
fn live_assessment_report_invariants() {
    // Defaults point at the local model router, so the run happens whenever
    // the bridge answers; an unreachable bridge skips the live test.
    let config = Config::from_env();
    let probe = Client::local()
        .with_base_url(config.bridge)
        .with_key(config.bridge_key);
    if probe.available_models().is_err() {
        eprintln!("bridge unreachable: live assessment skipped");
        return;
    }
    let target =
        std::env::var("ASSESS_TARGET").unwrap_or_else(|_| "host.docker.internal".to_string());
    let params = AssessParams {
        target,
        agent: None,
        model: std::env::var("ASSESS_MODEL").ok(),
        max_hypotheses: None,
        pattern: None,
    };
    let report = ok_or_fail(assess(&params), "live assessment");

    assert_eq!(report.schema, "maestro-security/report/3");
    assert!(!report.run_id.trim().is_empty());
    assert!(
        !report.model.trim().is_empty(),
        "model resolved at run time"
    );
    let names: Vec<&str> = report.phases.iter().map(|p| p.name.as_str()).collect();
    assert_phases(&names);
    for finding in &report.findings {
        assert_finding(finding);
    }
    assert!(!report.summary.trim().is_empty());

    let ledger = ok_or_fail(evidence_for(&report.run_id), "evidence retention");
    assert!(!ledger.trim().is_empty());

    let out_dir = std::env::var("ASSESS_OUT")
        .unwrap_or_else(|_| "/home/franc/workspace/maestro-kali/evidence".to_string());
    let path = format!("{out_dir}/unified-assessment-{}.json", report.run_id);
    let json = ok_or_fail(
        serde_json::to_string_pretty(&report),
        "report serialization",
    );
    ok_or_fail(std::fs::create_dir_all(&out_dir), "evidence dir");
    ok_or_fail(std::fs::write(&path, json), "report save");

    println!(
        "live assessment ok: run={} model={} findings={} refuted={} unknown={} report={}",
        report.run_id,
        report.model,
        report.findings.len(),
        report.refuted.len(),
        report.unknown.len(),
        path
    );
}
