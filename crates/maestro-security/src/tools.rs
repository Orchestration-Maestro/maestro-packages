//! Tool names, descriptions and JSON schemas: one copy, used by both the
//! extension surface and the MCP surface.

use serde_json::json;

/// Name of the assessment tool.
pub const ASSESS: &str = "security_assess";
/// Name of the evidence tool.
pub const EVIDENCE: &str = "security_evidence";

/// Description of the assessment tool.
pub const ASSESS_DESCRIPTION: &str = "Understand the target's program and network \
behaviour, find security issues and their root causes through the unified \
pipeline (recon, target \
model, hypothesis sweep, verify, deepen, report) against a target through the \
analysis sandbox runner and the bridge model. Pick an agent: maestro.security \
(default, full review pipeline), maestro.probe (evidence capture), \
maestro.proof (independent verification), maestro.rebuild (target \
reconstruction), maestro.cyber (threat model and hardening).";

/// Description of the evidence tool.
pub const EVIDENCE_DESCRIPTION: &str = "Retrieve the retained evidence ledger (full \
untruncated outputs of every step) for a finished run, by run id.";

/// JSON schema for the assessment tool parameters.
#[must_use]
pub fn assess_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "required": ["target"],
        "properties": {
            "target": { "type": "string", "description": "Host, IP or URL to assess" },
            "agent": {
                "type": "string",
                "enum": [
                    "maestro.security",
                    "maestro.probe",
                    "maestro.proof",
                    "maestro.rebuild",
                    "maestro.cyber"
                ],
                "description": "Agent role (default maestro.security)"
            },
            "model": {
                "type": "string",
                "description": "Optional model override; defaults to the bridge's first model"
            },
            "max_hypotheses": {
                "type": "integer",
                "description": "Maximum hypotheses to verify (default 8)"
            }
        }
    })
}

/// JSON schema for the evidence tool parameters.
#[must_use]
pub fn evidence_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "required": ["run_id"],
        "properties": {
            "run_id": {
                "type": "string",
                "description": "Run id from a finished assessment report"
            }
        }
    })
}
