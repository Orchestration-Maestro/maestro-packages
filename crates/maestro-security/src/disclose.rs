//! Coordinated disclosure: turn findings into a disclosure-ready document —
//! severity, the affected party, a recommended disclosure timeline, and a
//! draft report the owner can send.

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ctx::CapabilityCtx;
use crate::pipeline::Config;
use crate::report::Assessment;

/// Schema tag for the disclosure document.
pub const SCHEMA: &str = "maestro-security/disclose/1";

/// Parameters for a disclosure.
#[derive(Debug, Clone, Deserialize)]
pub struct Brief {
    /// The target being disclosed against.
    pub target: String,
    /// The findings to disclose, as text.
    pub findings: Vec<String>,
    /// Model override.
    pub model: Option<String>,
}

/// The disclosure document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Disclosure {
    /// Schema tag.
    pub schema: &'static str,
    /// The target.
    pub target: String,
    /// Overall severity: critical | high | medium | low.
    pub severity: String,
    /// Who should receive the disclosure.
    pub recipient: String,
    /// Recommended disclosure timeline.
    pub timeline: String,
    /// The draft disclosure report.
    pub draft: String,
}

/// Draft a coordinated disclosure for the given findings.
///
/// # Errors
/// Fails if the target is empty, no findings are given, or the model call
/// fails.
pub fn disclose(params: &Brief) -> Result<Disclosure, String> {
    if params.target.trim().is_empty() {
        return Err("target is required".into());
    }
    if params.findings.is_empty() {
        return Err("at least one finding is required".into());
    }
    let ctx = CapabilityCtx::build(&Config::from_env(), params.model.as_deref())?;
    let listing: String = params
        .findings
        .iter()
        .enumerate()
        .map(|(i, f)| format!("[{i}] {f}"))
        .collect::<Vec<_>>()
        .join("\n");
    let user = format!(
        "Target: {}\n\nFindings:\n{listing}\n\nDraft a coordinated disclosure. \
Return JSON: {{\"severity\":\"critical|high|medium|low\",\"recipient\":\
\"...\",\"timeline\":\"...\",\"draft\":\"...\"}}. The draft is a ready-to-send \
report: what was found, the impact, and the fix.",
        params.target
    );
    let raw = ctx.chat(
        "You are the disclosure writer. You write a clear, factual, ready-to-send \
coordinated disclosure.",
        &user,
        8192,
    )?;
    let json = crate::pipeline::extract_json(&raw);
    let value: serde_json::Value =
        serde_json::from_str(&json).map_err(|e| format!("disclose json: {e}"))?;
    Ok(Disclosure {
        schema: SCHEMA,
        target: params.target.clone(),
        severity: field(&value, "severity", "medium"),
        recipient: field(&value, "recipient", "the target operator"),
        timeline: field(&value, "timeline", "90 days"),
        draft: field(&value, "draft", &raw),
    })
}

/// Read a string field from a JSON value with a fallback.
fn field(value: &serde_json::Value, key: &str, fallback: &str) -> String {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or(fallback)
        .to_string()
}

/// Export the assessment as a SARIF 2.1.0 document: one result per finding,
/// keyed by its stable fingerprint.
#[must_use]
pub fn sarif(a: &Assessment) -> String {
    let results: Vec<serde_json::Value> = a
        .findings
        .iter()
        .map(|f| {
            json!({
                "ruleId": f.class,
                "level": sarif_level(&f.severity),
                "message": { "text": format!("{}: {}", f.title, f.impact) },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": { "uri": a.target }
                    }
                }],
                "fingerprints": { "maestro": f.fingerprint },
            })
        })
        .collect();
    let doc = json!({
        "version": "2.1.0",
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "runs": [{
            "tool": { "driver": {
                "name": "maestro-security",
                "version": env!("CARGO_PKG_VERSION"),
            } },
            "results": results,
        }],
    });
    serde_json::to_string(&doc).unwrap_or_default()
}

/// Map a finding severity to a SARIF level.
fn sarif_level(severity: &str) -> &'static str {
    match severity {
        "critical" | "high" => "error",
        "medium" => "warning",
        _ => "note",
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for disclosure helpers.

    use super::{field, sarif};
    use crate::report::{Assessment, Evidence, Execution, Finding, Verdict};

    /// A present field is returned; a missing one falls back.
    #[test]
    fn field_falls_back_when_missing() {
        let value = serde_json::json!({"a": "x"});
        assert_eq!(field(&value, "a", "d"), "x");
        assert_eq!(field(&value, "b", "d"), "d");
    }

    /// One confirmed finding exports as a SARIF 2.1.0 result carrying its
    /// fingerprint, class and target.
    #[test]
    fn sarif_exports_findings() {
        let finding = Finding {
            id: "F-001".into(),
            fingerprint: "example-com::injection::title".into(),
            title: "title".into(),
            class: "injection".into(),
            verdict: Verdict::Confirmed,
            severity: "high".into(),
            likelihood: "high".into(),
            confidence: 0.9,
            trace: vec!["entrypoint".into()],
            conditions: Vec::new(),
            execution: Some(Execution::default()),
            mitre: None,
            poc: None,
            evidence_level: "CONFIRMED".into(),
            escalation: String::new(),
            evidence: Evidence {
                command: "curl ...".into(),
                output: "ok".into(),
                id: "ev-0001".into(),
            },
            impact: "data".into(),
            fix: "fix".into(),
            target: "example.com".into(),
            claimed_root_cause: None,
            blockers: Vec::new(),
            validation_plan: None,
            reason: None,
        };
        let assessment = Assessment {
            schema: crate::report::SCHEMA.into(),
            run_id: "r1".into(),
            agent: "maestro.security".into(),
            target: "example.com".into(),
            model: "m".into(),
            phases: Vec::new(),
            findings: vec![finding],
            unknown: Vec::new(),
            refuted: Vec::new(),
            threats: Vec::new(),
            runtime_mechanics: Vec::new(),
            spend: None,
            summary: String::new(),
        };
        let doc: serde_json::Value = serde_json::from_str(&sarif(&assessment)).unwrap();
        assert_eq!(doc["version"], "2.1.0");
        assert_eq!(doc["runs"][0]["results"][0]["ruleId"], "injection");
        assert_eq!(doc["runs"][0]["results"][0]["level"], "error");
        assert_eq!(
            doc["runs"][0]["results"][0]["fingerprints"]["maestro"],
            "example-com::injection::title"
        );
        assert_eq!(
            doc["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
            "example.com"
        );
    }
}
