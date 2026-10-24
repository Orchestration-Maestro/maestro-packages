//! Pipeline orchestration: the unified methodology, run per agent.
//!
//! Recon executes deterministically through the runner; every other phase
//! is one bridge call whose output is ledgered with a sequential evidence
//! id. The model is resolved at run time, never stored. Phase
//! implementations live in [`crate::phases`].

use serde::{Deserialize, Serialize};

use crate::agents::Agent;
use crate::evidence::Ledger;
use crate::llm::Llm;
use crate::methodology;
use crate::report::{Evidence, Finding, Refuted, Report, Threat, REPORT_SCHEMA};
use crate::runner::Runner;

/// The canonical phase order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseKind {
    /// Phase 1: deterministic recon through the runner.
    Recon,
    /// Phase 2: structured target model.
    Model,
    /// Phase 3: breadth-first hypothesis sweep.
    Hypotheses,
    /// Phase 4: verify every hypothesis.
    Verify,
    /// Phase 5: deepen confirmed findings.
    Deepen,
    /// Threat phase: threat model and hardening (maestro.cyber).
    Threat,
    /// Phase 6: final report.
    Report,
}

/// Every phase, in canonical order.
const CANONICAL: [PhaseKind; 7] = [
    PhaseKind::Recon,
    PhaseKind::Model,
    PhaseKind::Hypotheses,
    PhaseKind::Verify,
    PhaseKind::Deepen,
    PhaseKind::Threat,
    PhaseKind::Report,
];

/// Pipeline endpoints and limits.
#[derive(Clone)]
pub struct Config {
    /// Bridge base URL including the audience segment.
    pub bridge: String,
    /// Bridge capability key.
    pub bridge_key: String,
    /// Runner base URL.
    pub runner: String,
    /// Maximum hypotheses verified per run.
    pub max_hypotheses: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bridge: crate::llm::DEFAULT_BASE_URL.into(),
            bridge_key: crate::llm::DEFAULT_KEY.into(),
            runner: crate::runner::DEFAULT_RUNNER_URL.into(),
            max_hypotheses: 8,
        }
    }
}

impl Config {
    /// Defaults overridden by the `MAESTRO_SECURITY_*` environment.
    #[must_use]
    pub fn from_env() -> Self {
        let mut config = Self::default();
        for (key, field) in [
            ("MAESTRO_SECURITY_BRIDGE", &mut config.bridge),
            ("MAESTRO_SECURITY_BRIDGE_KEY", &mut config.bridge_key),
            ("MAESTRO_SECURITY_RUNNER", &mut config.runner),
        ] {
            if let Ok(value) = std::env::var(key) && !value.is_empty() {
                *field = value;
            }
        }
        if let Ok(value) = std::env::var("MAESTRO_SECURITY_MAX_HYPOTHESES")
            && let Ok(n) = value.parse::<usize>()
            && n > 0
        {
            config.max_hypotheses = n;
        }
        config
    }
}

/// Parameters of one assessment run (shared by both surfaces).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AssessParams {
    /// Target to assess: host, IP, or URL.
    pub target: String,
    /// Agent role name; defaults to `maestro.pentester`.
    pub agent: Option<String>,
    /// Optional model override; defaults to the first model the bridge serves.
    pub model: Option<String>,
    /// Maximum hypotheses to verify (default 8).
    pub max_hypotheses: Option<usize>,
}

/// A phase-3 hypothesis as returned by the model.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct Hypothesis {
    /// Vulnerability class.
    pub(crate) class: String,
    /// The claim to test.
    pub(crate) claim: String,
    /// One non-destructive shell command that tests it.
    pub(crate) command: String,
    /// Output that would prove it.
    #[serde(default)]
    pub(crate) expected_if_true: String,
    /// Output that would refute it.
    #[serde(default)]
    pub(crate) refuted_if: String,
}

/// The model's phase-4 verification plan for one hypothesis.
#[derive(Debug, Deserialize)]
pub(crate) struct Plan {
    /// One shell command to run.
    pub(crate) command: String,
    /// A second command that would disprove a positive result.
    #[serde(default)]
    pub(crate) refutation_command: String,
}

/// The model's strict judgement of one execution.
#[derive(Debug, Deserialize)]
pub(crate) struct Judge {
    /// CONFIRMED | LIKELY | POSSIBLE | REFUTED.
    pub(crate) evidence_level: String,
    /// One or two sentences.
    #[serde(default)]
    pub(crate) reasoning: String,
    /// Looked for but not proven; what is missing.
    #[serde(default)]
    pub(crate) unknown: Vec<String>,
    /// What would disprove this, and whether it was tried.
    #[serde(default)]
    pub(crate) counterevidence: String,
}

/// Phase-5 deepening of one confirmed finding.
#[derive(Debug, Deserialize)]
pub(crate) struct Deepen {
    /// One step further the finding enables.
    pub(crate) escalation: String,
    /// Concrete data or action affected.
    pub(crate) impact: String,
    /// Minimal standard fix.
    pub(crate) fix: String,
    /// critical | high | medium | low | info.
    pub(crate) severity: String,
}

/// One judged verification attempt.
pub(crate) struct Verification {
    /// The hypothesis that was tested.
    pub(crate) hypothesis: Hypothesis,
    /// The command that was executed.
    pub(crate) command: String,
    /// The observed output, truncated for the report.
    pub(crate) output: String,
    /// The ledger entry that retains the full output.
    pub(crate) evidence_id: String,
    /// CONFIRMED | LIKELY | POSSIBLE | REFUTED.
    pub(crate) level: String,
    /// The model's judgement in one or two sentences.
    pub(crate) reasoning: String,
    /// Counterevidence: the model's claim plus the executed refutation.
    pub(crate) counterevidence: String,
    /// Looked for but not proven.
    pub(crate) unknown: Vec<String>,
}

/// The mutable state of one run.
pub(crate) struct RunState {
    /// The assessed target.
    pub(crate) target: String,
    /// The evidence ledger for the run.
    pub(crate) ledger: Ledger,
    /// Phase-1 raw recon output.
    pub(crate) recon: String,
    /// Phase-2 structured target model.
    pub(crate) target_model: String,
    /// Phase-3 hypotheses, consumed by phase 4.
    pub(crate) hypotheses: Vec<Hypothesis>,
    /// Judged verification attempts.
    pub(crate) verifications: Vec<Verification>,
    /// Findings, deepened in phase 5.
    pub(crate) findings: Vec<Finding>,
    /// Refuted hypotheses (coverage proof).
    pub(crate) refuted: Vec<Refuted>,
    /// Threat model entries (maestro.cyber).
    pub(crate) threats: Vec<Threat>,
    /// Next finding number.
    pub(crate) f_next: u32,
}

impl RunState {
    /// Build the state for one target.
    pub(crate) fn new(target: &str) -> Self {
        Self {
            target: target.into(),
            ledger: Ledger::new(),
            recon: String::new(),
            target_model: String::new(),
            hypotheses: Vec::new(),
            verifications: Vec::new(),
            findings: Vec::new(),
            refuted: Vec::new(),
            threats: Vec::new(),
            f_next: 1,
        }
    }

    /// The next finding number, then advance the counter.
    pub(crate) fn next_f_id(&mut self) -> u32 {
        let id = self.f_next;
        self.f_next += 1;
        id
    }
}

/// The model context for one run: the agent's system prompt and the model
/// resolved at run time.
#[derive(Clone)]
pub(crate) struct ModelCtx {
    /// The unified system prompt plus the agent's role.
    pub(crate) system: String,
    /// The model resolved at run time from the bridge.
    pub(crate) model: String,
}

/// The unified pipeline bound to one configuration.
pub struct Pipeline {
    /// The bridge client.
    pub(crate) llm: Llm,
    /// The runner client.
    pub(crate) runner: Runner,
    /// Endpoints and limits.
    pub(crate) config: Config,
}

impl Pipeline {
    /// Build a pipeline from a configuration.
    #[must_use]
    pub fn new(config: Config) -> Self {
        let llm = Llm::local()
            .with_base_url(config.bridge.clone())
            .with_key(config.bridge_key.clone());
        let runner = Runner::local().with_base_url(config.runner.clone());
        Self {
            llm,
            runner,
            config,
        }
    }

    /// Run one agent's phases against a target.
    ///
    /// # Errors
    /// Fails if the runner preflight fails, the bridge serves no model, or
    /// any phase fails.
    pub fn run(
        &self,
        agent: &Agent,
        target: &str,
        model: Option<&str>,
    ) -> Result<Report, String> {
        let run_id = new_run_id(target);
        self.runner
            .health()
            .map_err(|error| format!("runner preflight failed: {error}"))?;
        let ctx = ModelCtx {
            system: system_prompt(agent),
            model: self.resolve_model(model)?,
        };
        let mut state = RunState::new(target);
        for phase in CANONICAL {
            if !agent.phases.contains(&phase) {
                continue;
            }
            self.execute(phase, &ctx, &mut state)?;
        }
        self.assemble(&run_id, agent, &ctx, &mut state)
    }

    /// Dispatch one phase.
    fn execute(
        &self,
        phase: PhaseKind,
        ctx: &ModelCtx,
        state: &mut RunState,
    ) -> Result<(), String> {
        match phase {
            PhaseKind::Recon => {
                state.recon = self.phase_recon(&state.target, &mut state.ledger)?
            }
            PhaseKind::Model => {
                state.target_model = self.phase_target_model(
                    ctx,
                    &state.target,
                    &state.recon,
                    &mut state.ledger,
                )?
            }
            PhaseKind::Hypotheses => {
                state.hypotheses =
                    self.phase_hypotheses(ctx, &state.target_model, &mut state.ledger)
            }
            PhaseKind::Verify => self.phase_verify(ctx, state)?,
            PhaseKind::Deepen => self.phase_deepen(ctx, state)?,
            PhaseKind::Threat => {
                state.threats =
                    self.phase_threat(ctx, &state.target_model, &mut state.ledger)
            }
            PhaseKind::Report => {}
        }
        Ok(())
    }

    /// Assemble the report, run the report phase, retain the ledger.
    fn assemble(
        &self,
        run_id: &str,
        agent: &Agent,
        ctx: &ModelCtx,
        state: &mut RunState,
    ) -> Result<Report, String> {
        let unknown = collect_unknown(&state.verifications);
        let (summary, mechanics) = if agent.phases.contains(&PhaseKind::Report) {
            let data = serde_json::json!({
                "target": state.target,
                "target_model": truncate(&state.target_model, 8000),
                "findings": state.findings,
                "refuted": state.refuted,
            })
            .to_string();
            self.phase_summary(ctx, &data, &mut state.ledger)
        } else {
            (
                default_summary(&state.target, &state.findings, &state.refuted),
                Vec::new(),
            )
        };
        let report = Report {
            schema: REPORT_SCHEMA,
            run_id: run_id.into(),
            agent: agent.name.into(),
            target: state.target.clone(),
            model: ctx.model.clone(),
            phases: state.ledger.phases().to_vec(),
            findings: std::mem::take(&mut state.findings),
            unknown,
            refuted: std::mem::take(&mut state.refuted),
            threats: std::mem::take(&mut state.threats),
            runtime_mechanics: mechanics,
            summary,
        };
        state.ledger.save(run_id, &state.target, agent.name, &ctx.model)?;
        Ok(report)
    }

    /// Resolve the run's model: explicit override, else the bridge's first.
    fn resolve_model(&self, override_model: Option<&str>) -> Result<String, String> {
        if let Some(model) = override_model {
            return Ok(model.to_string());
        }
        self.llm.available_models().and_then(|models| {
            models
                .into_iter()
                .next()
                .ok_or_else(|| "bridge /models returned no models".to_string())
        })
    }
}

/// Run one assessment with a configuration from the environment.
///
/// # Errors
/// Fails if the target is empty or the pipeline run fails.
pub fn assess(params: &AssessParams) -> Result<Report, String> {
    if params.target.trim().is_empty() {
        return Err("target is required".into());
    }
    let mut config = Config::from_env();
    if let Some(n) = params.max_hypotheses && n > 0 {
        config.max_hypotheses = n;
    }
    let agent = crate::agents::resolve_agent(params.agent.as_deref());
    let pipeline = Pipeline::new(config);
    pipeline.run(agent, &params.target, params.model.as_deref())
}

/// Retrieve the retained evidence ledger for a finished run.
///
/// # Errors
/// Fails if no ledger was retained for the run id.
pub fn evidence_for(run_id: &str) -> Result<String, String> {
    crate::evidence::load(run_id)
}

/// The system prompt for one agent: the unified prompt plus the role.
fn system_prompt(agent: &Agent) -> String {
    format!(
        "{}\n\nAGENT ROLE:\n{}",
        methodology::SYSTEM_PROMPT, agent.role
    )
}

/// A filename-safe run id: `run-<unix-seconds>-<target-slug>`.
fn new_run_id(target: &str) -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let slug: String = target
        .chars()
        .take(24)
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    format!("run-{secs}-{slug}")
}

/// Deduplicated unknowns across all verifications.
fn collect_unknown(verifications: &[Verification]) -> Vec<String> {
    let mut unknown: Vec<String> = verifications
        .iter()
        .flat_map(|v| v.unknown.iter().cloned())
        .collect();
    unknown.sort();
    unknown.dedup();
    unknown
}

/// A verified finding before deepening: level and evidence only.
pub(crate) fn shallow_finding(target: &str, v: &Verification, id: u32) -> Finding {
    Finding {
        id: format!("F-{id:03}"),
        title: truncate(&v.hypothesis.claim, 90).to_string(),
        class: v.hypothesis.class.clone(),
        severity: "info".into(),
        evidence_level: v.level.clone(),
        escalation: String::new(),
        evidence: Evidence {
            command: v.command.clone(),
            output: v.output.clone(),
            evidence_id: v.evidence_id.clone(),
        },
        impact: v.reasoning.clone(),
        fix: String::new(),
        target: target.into(),
    }
}

/// Deterministic fallback summary from the report data.
pub(crate) fn default_summary_for(report_data: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(report_data).unwrap_or_default();
    let findings = value
        .get("findings")
        .and_then(|f| f.as_array())
        .map(Vec::len)
        .unwrap_or(0);
    let refuted = value
        .get("refuted")
        .and_then(|r| r.as_array())
        .map(Vec::len)
        .unwrap_or(0);
    format!(
        "Assessment finished. {findings} findings, {refuted} hypotheses refuted. \
See the findings list for evidence ids."
    )
}

/// Deterministic fallback summary for agents without a report phase.
fn default_summary(target: &str, findings: &[Finding], refuted: &[Refuted]) -> String {
    format!(
        "Assessed {target}. {} findings ({} confirmed), {} hypotheses refuted. \
See the findings list for evidence ids.",
        findings.len(),
        findings
            .iter()
            .filter(|f| f.evidence_level == "CONFIRMED")
            .count(),
        refuted.len()
    )
}

/// Pull the outermost JSON object out of a model response.
pub(crate) fn extract_json(raw: &str) -> String {
    let start = raw.find('{').unwrap_or(0);
    let end = raw.rfind('}').map(|i| i + 1).unwrap_or(raw.len());
    if start < end {
        raw[start..end].to_string()
    } else {
        raw.to_string()
    }
}

/// Truncate at a char boundary without panicking.
pub(crate) fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

#[cfg(test)]
mod tests {
    //! Unit tests for the pipeline helpers.

    use super::{extract_json, new_run_id, truncate};

    /// `extract_json` keeps the outermost braces even with noise around.
    #[test]
    fn extract_json_pulls_the_outermost_object() {
        let raw = "noise {\"a\":1} tail {\"b\":2} end";
        assert_eq!(extract_json(raw), "{\"a\":1} tail {\"b\":2}");
    }

    /// `truncate` stops on a char boundary, never mid-character.
    #[test]
    fn truncate_never_splits_a_char() {
        let s = "éééé";
        assert_eq!(truncate(s, 5), "éé");
    }

    /// Run ids carry no path separators or query characters.
    #[test]
    fn run_id_is_filename_safe() {
        let id = new_run_id("https://example.com:8080/x?y=1");
        assert!(id.starts_with("run-"));
        assert!(!id.contains(':') && !id.contains('/') && !id.contains('?'));
    }

    /// Unknown agent names fall back to the default pentester.
    #[test]
    fn unknown_agent_falls_back_to_the_default() {
        assert_eq!(
            crate::agents::resolve_agent(Some("nope")).name,
            "maestro.pentester"
        );
    }
}
