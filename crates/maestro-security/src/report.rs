//! Assessment types: the machine-readable output of one security run.
//!
//! The schema is versioned in `Assessment::schema` so future pipeline changes
//! can add fields without breaking stored reports.

use serde::{Deserialize, Serialize};

/// Schema tag for the report document.
pub const SCHEMA: &str = "maestro-security/report/3";

/// A finding's verdict: what the validation actually established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Source-grounded and independently demonstrated.
    Confirmed,
    /// A candidate whose decisive validation was blocked or not yet run.
    NeedsValidation,
    /// A candidate refuted during validation.
    Rejected,
}

/// One executed step of the pipeline, with its evidence id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Phase {
    /// Phase name: recon, target-model, hypotheses, verify, deepen, threat, report.
    pub name: String,
    /// Engine-assigned evidence id for this step's output.
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
    /// The ledger entry retaining the full output.
    pub id: String,
}

/// How an attacker would use a confirmed finding.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Execution {
    /// What the attacker does first, in their words.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub attacker_perspective: String,
    /// Concrete payloads or commands that demonstrate it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub payloads: Vec<String>,
    /// Step-by-step instructions to reproduce.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub instructions: String,
    /// What was actually observed when it was run.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub observed_result: String,
}

/// A generated proof-of-concept and whether it fired.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoC {
    /// The safe, non-destructive script.
    pub script: String,
    /// True only if the script was run and actually fired.
    pub verified: bool,
    /// What the run showed.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub observed: String,
}

/// One finding in the final report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// Stable id, e.g. `F-001`.
    pub id: String,
    /// Stable fingerprint: target + class + title slug. Unique and sorted.
    pub fingerprint: String,
    /// Short title.
    pub title: String,
    /// Vulnerability class.
    pub class: String,
    /// What the validation established.
    pub verdict: Verdict,
    /// critical | high | medium | low | info.
    pub severity: String,
    /// low | medium | high: how easily an attacker reaches it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub likelihood: String,
    /// Pheromone confidence in \[0,1\]: 0.1 superseded, 0.5+ publishable.
    pub confidence: f64,
    /// Path from entrypoint through propagation to sink.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trace: Vec<String>,
    /// Conditions under which it holds (auth, network, timing, ...).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<String>,
    /// How an attacker would use it (confirmed findings).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution: Option<Execution>,
    /// ATT&CK technique id, e.g. `T1078`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mitre: Option<String>,
    /// The proof-of-concept, if one was generated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poc: Option<PoC>,
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
    /// The root cause as claimed before validation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claimed_root_cause: Option<String>,
    /// What blocked decisive validation (`needs_validation`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blockers: Vec<String>,
    /// How to validate once the blocker clears (`needs_validation`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_plan: Option<String>,
    /// Why the candidate was refuted (rejected).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
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

/// What one run cost: the ROI footer of the report.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunSpend {
    /// Wall-clock duration in seconds.
    pub duration_secs: u64,
    /// LLM calls made.
    pub llm_calls: usize,
    /// Output tokens reported by the bridge.
    pub tokens_out: u64,
}

/// The final report of one run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assessment {
    /// Schema tag.
    pub schema: String,
    /// Run id; keys the retained evidence ledger.
    pub run_id: String,
    /// Agent that ran, e.g. `maestro.security`.
    pub agent: String,
    /// The assessed target.
    pub target: String,
    /// The model that ran the pipeline (resolved at run time).
    pub model: String,
    /// Engine-assigned evidence ids, in execution order.
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
    /// What the run cost.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spend: Option<RunSpend>,
    /// Three-sentence summary.
    pub summary: String,
}
