//! Baseline health reports through the public doctor entry point.

use crate::support;
use maestro_security::doctor::{Classification, doctor};
use serde_json::Value;

/// Reachable dependencies with the required tools and CPU-only hardware.
fn healthy(path: &str, body: &Value) -> (u16, String) {
    match path {
        "/health" => (200, String::new()),
        "/models" => support::models(),
        "/chat/completions" => support::completion("OK"),
        "/exec"
            if body["command"]
                .as_str()
                .unwrap_or("")
                .starts_with("command") =>
        {
            support::execution("/bin/nmap\n/bin/grep\n/bin/find\n", 0)
        }
        _ => support::execution("none\n", 0),
    }
}

/// Baseline: no GPU is healthy CPU-only, with no remediation required.
#[test]
fn healthy_cpu_only_has_no_failures() -> Result<(), Box<dyn std::error::Error>> {
    if !support::isolated("doctor::healthy_cpu_only_has_no_failures", healthy)? {
        return Ok(());
    }
    let report = doctor()?;
    assert_eq!(report.schema, "maestro-security/doctor/2");
    assert_eq!((report.passed, report.failed), (6, 0));
    assert!(report.checks.iter().all(|check| check.status == "pass"
        && check.classification == Classification::Healthy
        && check.remediation.is_empty()));
    assert_eq!(
        report
            .checks
            .iter()
            .map(|check| check.name.as_str())
            .collect::<Vec<_>>(),
        [
            "runner",
            "bridge",
            "models",
            "inference",
            "tooling",
            "hardware"
        ]
    );
    assert_eq!(report.checks[5].detail, "cpu-only");
    Ok(())
}

/// Deny network checks and report failed tool/device probes.
fn unavailable(path: &str, body: &Value) -> (u16, String) {
    if path == "/exec" {
        let tooling = body["command"]
            .as_str()
            .unwrap_or("")
            .starts_with("command");
        support::execution("", if tooling { 127 } else { 1 })
    } else {
        (403, "fixture unavailable".into())
    }
}

/// Baseline: failures are reported individually with actionable classifications.
#[test]
fn failures_are_classified_without_aborting() -> Result<(), Box<dyn std::error::Error>> {
    if !support::isolated(
        "doctor::failures_are_classified_without_aborting",
        unavailable,
    )? {
        return Ok(());
    }
    let report = doctor()?;
    assert_eq!((report.passed, report.failed), (0, 6));
    let classifications: Vec<_> = report
        .checks
        .iter()
        .map(|check| check.classification)
        .collect();
    assert_eq!(
        classifications,
        [
            Classification::ConfigDrift,
            Classification::ConfigDrift,
            Classification::ConfigDrift,
            Classification::MissingAnalysisEngine,
            Classification::MissingAnalysisEngine,
            Classification::UnsupportedHost
        ]
    );
    assert!(
        report
            .checks
            .iter()
            .all(|check| check.status == "fail" && !check.remediation.is_empty())
    );
    assert!(report.checks[4].detail.contains("127"));
    assert!(report.checks[5].detail.contains('1'));
    Ok(())
}

/// Healthy services with two usable GPU devices and an irrelevant banner.
fn gpu(path: &str, body: &Value) -> (u16, String) {
    if path == "/exec" && body["command"].as_str().unwrap_or("").starts_with("nvidia") {
        support::execution("driver banner\nGPU 0: device A\nGPU 1: device B\n", 0)
    } else {
        healthy(path, body)
    }
}

/// Baseline: usable GPU devices are counted without counting probe banners.
#[test]
fn gpu_probe_counts_devices() -> Result<(), Box<dyn std::error::Error>> {
    if !support::isolated("doctor::gpu_probe_counts_devices", gpu)? {
        return Ok(());
    }
    let report = doctor()?;
    assert_eq!(report.checks[5].detail, "2 gpu device(s)");
    assert_eq!(report.checks[5].classification, Classification::Healthy);
    Ok(())
}

/// Baseline: runner transport failures classify both probes as configuration drift.
#[test]
fn runner_probe_errors_are_drift() -> Result<(), Box<dyn std::error::Error>> {
    if !support::isolated("doctor::runner_probe_errors_are_drift", denied)? {
        return Ok(());
    }
    let report = doctor()?;
    assert_eq!(report.checks.len(), 6);
    for (check, name) in report.checks[4..].iter().zip(["tooling", "hardware"]) {
        assert_eq!(check.name, name);
        assert_eq!(check.classification, Classification::ConfigDrift);
        assert!(check.detail.contains("403"));
        assert!(!check.remediation.is_empty());
    }
    Ok(())
}

/// Deny only runner executions while leaving all other services healthy.
fn denied(path: &str, body: &Value) -> (u16, String) {
    if path == "/exec" {
        (403, "probe denied".into())
    } else {
        healthy(path, body)
    }
}
