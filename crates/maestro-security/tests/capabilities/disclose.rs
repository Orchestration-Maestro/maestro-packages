//! Baseline disclosure drafting and SARIF export through public APIs.

use maestro_security::Assessment;
use maestro_security::disclose::{Brief, disclose, sarif};
use serde_json::{Value, json};

use super::support::{Reply, Result, case};

/// Baseline: disclosure requires both a target and findings.
#[test]
fn requires_target_and_findings() {
    let mut params = brief();
    params.target = " \n".into();
    assert_eq!(
        disclose(&params).err().as_deref(),
        Some("target is required")
    );
    params.target = "example.test".into();
    params.findings.clear();
    assert_eq!(
        disclose(&params).err().as_deref(),
        Some("at least one finding is required")
    );
}

/// Baseline: structured model fields become a disclosure for the requested target.
#[test]
fn drafts_disclosure() -> Result {
    case(
        "disclose::drafts_disclosure",
        vec![Reply::chat(
            "```json\n{\"severity\":\"high\",\"recipient\":\"security@example.test\",\"timeline\":\"30 days\",\"draft\":\"Reproduce with the attached probe.\"}\n```",
        )],
        || {
            let report = disclose(&brief())?;
            assert_eq!(report.target, "example.test");
            assert_eq!(report.severity, "high");
            assert_eq!(report.recipient, "security@example.test");
            assert_eq!(report.timeline, "30 days");
            assert_eq!(report.draft, "Reproduce with the attached probe.");
            Ok(())
        },
    )
}

/// Baseline: missing or wrongly typed fields use disclosure defaults and retain the raw draft.
#[test]
fn incomplete_answers_use_defaults() -> Result {
    let answer = r#"{"severity":null,"recipient":7,"timeline":false}"#;
    case(
        "disclose::incomplete_answers_use_defaults",
        vec![Reply::chat(answer)],
        || {
            let report = disclose(&brief())?;
            assert_eq!(report.severity, "medium");
            assert_eq!(report.recipient, "the target operator");
            assert_eq!(report.timeline, "90 days");
            assert_eq!(report.draft, answer);
            Ok(())
        },
    )
}

/// Baseline: malformed JSON and model service errors are returned to the caller.
#[test]
fn propagates_bad_answers_and_service_failures() -> Result {
    case(
        "disclose::propagates_bad_answers_and_service_failures",
        vec![Reply::chat("not JSON"), Reply::failure("/chat/completions")],
        || {
            let malformed = disclose(&brief()).err().ok_or("expected JSON error")?;
            assert!(malformed.starts_with("disclose json:"));
            let service = disclose(&brief()).err().ok_or("expected service error")?;
            assert!(service.starts_with("bridge http 403"));
            Ok(())
        },
    )
}

/// Baseline: SARIF retains stable finding identities and maps every severity category.
#[test]
fn sarif_preserves_findings_and_severity_levels() -> Result {
    let mut assessment = assessment()?;
    let levels = [
        ("critical", "error"),
        ("high", "error"),
        ("medium", "warning"),
        ("low", "note"),
        ("info", "note"),
    ];
    for (index, (severity, _)) in levels.iter().enumerate() {
        assessment.findings.push(serde_json::from_value(json!({
            "id": format!("F-{index}"), "fingerprint": format!("fingerprint-{index}"),
            "title":"Unsafe query", "class":"injection", "verdict":"confirmed",
            "severity":severity, "confidence":0.9, "evidence_level":"CONFIRMED",
            "escalation":"", "evidence":{"command":"probe", "output":"row", "id":"ev-1"},
            "impact":"Read private data", "fix":"Bind parameters", "target":"example.test"
        }))?);
    }
    let document: Value = serde_json::from_str(&sarif(&assessment))?;
    assert_eq!(document["version"], "2.1.0");
    let results = document["runs"][0]["results"]
        .as_array()
        .ok_or("missing results")?;
    assert_eq!(results.len(), levels.len());
    for (index, (_, expected)) in levels.iter().enumerate() {
        let result = &results[index];
        assert_eq!(result["level"], *expected);
        assert_eq!(result["ruleId"], "injection");
        assert_eq!(
            result["fingerprints"]["maestro"],
            format!("fingerprint-{index}")
        );
        assert_eq!(result["message"]["text"], "Unsafe query: Read private data");
        assert_eq!(
            result["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
            "example.test"
        );
    }
    Ok(())
}

/// Baseline: exporting an assessment with no findings produces an empty result list.
#[test]
fn sarif_empty_assessment() -> Result {
    let document: Value = serde_json::from_str(&sarif(&assessment()?))?;
    assert_eq!(document["runs"][0]["results"], json!([]));
    Ok(())
}

/// A finding-free assessment suitable for the public SARIF exporter.
fn assessment() -> std::result::Result<Assessment, serde_json::Error> {
    serde_json::from_value(json!({
        "schema":"maestro-security/report/1", "run_id":"fixture-run", "agent":"security",
        "target":"example.test", "model":"fixture-model", "phases":[], "findings":[],
        "unknown":[], "refuted":[], "summary":""
    }))
}

/// A disclosure request with one finding and an explicit fixture model.
fn brief() -> Brief {
    Brief {
        target: "example.test".into(),
        findings: vec!["Unauthenticated access to private records".into()],
        model: Some("fixture-model".into()),
    }
}
