//! Report types: the machine-readable output of one security run.
//!
//! The schema is versioned in `Report::schema` so future pipeline changes
//! can add fields without breaking stored reports.

use serde::{Deserialize, Serialize};

/// Schema tag for the report document.
pub const REPORT_SCHEMA: &str = "maestro-security/report/2";

/// One executed step of the pipeline, with its evidence id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Phase {
    /// Phase name: recon, target-model, hypotheses, verify, deepen, threat, report.
    pub name: String,
    /// Pipeline-assigned evidence id for this step's output.
    pub evidence_id: String,
    /// One-line note on what the step produced.
    pub note: String,
}

/// The evidence attached to a finding: command, output excerpt, evidence id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    /// The exact command that produced the evidence.
    pub command: String,
    /// Output excerpt (bounded).
    pub output: String,
    /// Pipeline-assigned evidence id.
    pub evidence_id: String,
}

/// One finding in the final report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// Stable id, e.g. `F-001`.
    pub id: String,
    /// Short title.
    pub title: String,
    /// Vulnerability class.
    pub class: String,
    /// critical | high | medium | low | info.
    pub severity: String,
    /// CONFIRMED or LIKELY (POSSIBLE findings are listed, not findings).
    pub evidence_level: String,
    /// One step further the finding enables.
    pub escalation: String,
    /// The evidence that proves it.
    pub evidence: Evidence,
    /// Concrete impact: which data or action is affected.
    pub impact: String,
    /// Minimal standard fix.
    pub fix: String,
    /// The target node this applies to.
    pub target: String,
}

/// A hypothesis that was tested and refuted; listed to prove coverage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Refuted {
    /// Vulnerability class.
    pub class: String,
    /// The hypothesis as stated in phase 3.
    pub hypothesis: String,
    /// Evidence id of the refuting execution.
    pub evidence_id: String,
}

/// One threat-model entry (maestro.cyber).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Threat {
    /// The asset at risk.
    pub asset: String,
    /// Who wants it and why.
    pub actor: String,
    /// The exposure path.
    pub path: String,
    /// Concrete impact if taken.
    pub impact: String,
    /// low | medium | high.
    pub likelihood: String,
    /// Minimal standard hardening for this path.
    pub hardening: String,
}

/// The final report of one run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    /// Schema tag.
    pub schema: &'static str,
    /// Run id; keys the retained evidence ledger.
    pub run_id: String,
    /// Agent that ran, e.g. `maestro.security`.
    pub agent: String,
    /// The assessed target.
    pub target: String,
    /// The model that ran the pipeline (resolved at run time).
    pub model: String,
    /// Pipeline-assigned evidence ids, in execution order.
    pub phases: Vec<Phase>,
    /// Confirmed and likely findings, deepest first.
    pub findings: Vec<Finding>,
    /// Looked for, not proven: named gaps, not guesses.
    pub unknown: Vec<String>,
    /// Refuted hypotheses (coverage proof).
    pub refuted: Vec<Refuted>,
    /// Threat model entries (maestro.cyber only).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub threats: Vec<Threat>,
    /// Observed mechanics that should not be imitated.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runtime_mechanics: Vec<String>,
    /// Three-sentence summary.
    pub summary: String,
}
