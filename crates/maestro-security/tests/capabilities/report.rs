//! Baseline wire compatibility of stored assessment reports.

use maestro_security::{Assessment, Verdict};
use serde_json::{Value, json};

/// A report written without optional fields, as a stored-report reader receives it.
fn minimal() -> Value {
    json!({
        "schema":"maestro-security/report/3", "run_id":"run-31", "agent":"maestro.security",
        "target":"fixture-host", "model":"primary", "phases":[], "findings":[{
            "id":"F-001", "fingerprint":"fixture-host::injection::login", "title":"Login injection",
            "class":"injection", "verdict":"needs_validation", "severity":"high", "confidence":0.6,
            "evidence_level":"LIKELY", "escalation":"read accounts", "evidence":{
                "command":"replay request", "output":"query error", "id":"E-001"
            }, "impact":"account data", "fix":"bind parameters", "target":"fixture-host"
        }], "unknown":["credentials unavailable"], "refuted":[], "summary":"Validation blocked."
    })
}

/// Baseline: old minimal documents receive defaults, and empty optional fields stay omitted.
#[test]
fn minimal_stored_report_defaults_optional_fields() -> Result<(), Box<dyn std::error::Error>> {
    let report: Assessment = serde_json::from_value(minimal())?;
    assert_eq!(report.findings[0].verdict, Verdict::NeedsValidation);
    assert!(report.findings[0].execution.is_none());
    assert!(report.findings[0].blockers.is_empty());
    assert!(report.spend.is_none());
    assert!(report.threats.is_empty());
    let output = serde_json::to_value(&report)?;
    for field in ["execution", "poc", "trace", "blockers", "reason"] {
        assert!(output["findings"][0].get(field).is_none(), "{field}");
    }
    assert!(output.get("spend").is_none());
    assert!(output.get("threats").is_none());
    assert_eq!(output["findings"][0]["evidence"]["id"], "E-001");
    Ok(())
}

/// Baseline: enriched reports retain reproduction, provenance, threat and spend data.
#[test]
fn enriched_report_preserves_evidence_and_reproduction() -> Result<(), Box<dyn std::error::Error>> {
    let mut document = minimal();
    let finding = &mut document["findings"][0];
    finding["verdict"] = json!("confirmed");
    finding["likelihood"] = json!("high");
    finding["trace"] = json!(["request", "query"]);
    finding["conditions"] = json!(["authenticated"]);
    finding["execution"] = json!({"attacker_perspective":"read accounts", "payloads":["' OR 1=1"],
        "instructions":"replay with payload", "observed_result":"two accounts"});
    finding["poc"] = json!({"script":"replay", "verified":true, "observed":"two accounts"});
    finding["mitre"] = json!("T1190");
    finding["claimed_root_cause"] = json!("concatenated query");
    finding["blockers"] = json!(["production access"]);
    finding["validation_plan"] = json!("repeat against staging");
    finding["reason"] = json!("historical candidate retained");
    document["phases"] = json!([{"name":"verify", "evidence_id":"E-001", "note":"replayed"}]);
    document["refuted"] =
        json!([{"class":"xss", "hypothesis":"unescaped output", "evidence_id":"E-002"}]);
    document["threats"] = json!([{"asset":"accounts", "actor":"user", "path":"login", "impact":"disclosure",
        "likelihood":"high", "hardening":"parameterize"}]);
    document["runtime_mechanics"] = json!(["transport retried"]);
    document["spend"] = json!({"duration_secs":12, "llm_calls":3, "tokens_out":200});
    let report: Assessment = serde_json::from_value(document.clone())?;
    assert_eq!(report.findings[0].verdict, Verdict::Confirmed);
    assert_eq!(serde_json::to_value(report)?, document);
    Ok(())
}

/// Baseline: invalid verdicts and missing required evidence cannot become reports.
#[test]
fn invalid_verdict_and_missing_evidence_are_rejected() {
    let mut document = minimal();
    document["findings"][0]["verdict"] = json!("likely");
    assert!(serde_json::from_value::<Assessment>(document.clone()).is_err());
    document["findings"][0]["verdict"] = json!("rejected");
    assert!(serde_json::from_value::<Assessment>(document.clone()).is_ok());
    document["findings"][0]["evidence"] = Value::Null;
    assert!(serde_json::from_value::<Assessment>(document.clone()).is_err());
    document["findings"][0]
        .as_object_mut()
        .unwrap()
        .shift_remove("evidence");
    assert!(serde_json::from_value::<Assessment>(document).is_err());
}
