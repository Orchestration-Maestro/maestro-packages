//! Engine orchestration: the unified methodology, run per agent.
//!
//! Recon executes deterministically through the runner; every other phase
//! is one bridge call whose output is ledgered with a sequential evidence
//! id. The model is resolved at run time, never stored. Phase
//! implementations live in [`crate::phases`].

use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::agents::{Agent, Pattern};
use crate::evidence::Ledger;
use crate::llm::Client;
use crate::methodology;
use crate::report::{
    Evidence, Execution, Finding, Refuted, Verdict, Assessment, Threat, SCHEMA,
};
use crate::runner::Sandbox;

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

impl PhaseKind {
    /// The phase's name in the report and the evidence ledger.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Recon => "recon",
            Self::Model => "target-model",
            Self::Hypotheses => "hypotheses",
            Self::Verify => "verify",
            Self::Deepen => "deepen",
            Self::Threat => "threat",
            Self::Report => "report",
        }
    }
}

/// The phases one agent runs, in canonical order.
pub(crate) fn planned_phases(agent: &Agent) -> impl Iterator<Item = PhaseKind> + '_ {
    CANONICAL
        .into_iter()
        .filter(move |phase| agent.phases.contains(phase))
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

/// Engine endpoints and limits.
#[derive(Clone)]
pub struct Config {
    /// Bridge base URL including the audience segment.
    pub bridge: String,
    /// Bridge capability key.
    pub bridge_key: String,
    /// Sandbox base URL.
    pub runner: String,
    /// Default model when a run does not name one; used only if the bridge
    /// actually serves it, otherwise the bridge's first model.
    pub default_model: String,
    /// Maximum hypotheses verified per run.
    pub max_hypotheses: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bridge: crate::llm::DEFAULT_BASE_URL.into(),
            bridge_key: crate::llm::DEFAULT_KEY.into(),
            runner: crate::runner::DEFAULT_RUNNER_URL.into(),
            default_model: "qwythos".into(),
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
            ("MAESTRO_SECURITY_MODEL", &mut config.default_model),
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
    /// Agent role name; defaults to `maestro.security`.
    pub agent: Option<String>,
    /// Optional model override; defaults to the first model the bridge serves.
    pub model: Option<String>,
    /// Maximum hypotheses to verify (default 8).
    pub max_hypotheses: Option<usize>,
    /// The operational pattern: `red_team` (default), `blue_team`,
    /// `bug_bounter`, `compliance`, `continuous_ops`, or `dfir`.
    #[serde(default)]
    pub pattern: Option<String>,
}

/// A checkpoint callback: persist the ledger after each phase.
pub type Checkpoint = dyn FnMut(&Ledger, &str, &str);

/// Options for one engine run.
#[derive(Default)]
pub struct RunOptions<'a> {
    /// Model override; the bridge default when absent.
    pub model: Option<String>,
    /// The operational pattern.
    pub pattern: Pattern,
    /// Per-phase ledger persistence; `None` for one-shot runs.
    pub checkpoint: Option<&'a mut Checkpoint>,
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
    /// low | medium | high: how easily an attacker reaches it.
    #[serde(default)]
    pub(crate) likelihood: String,
    /// Path from entrypoint through propagation to sink.
    #[serde(default)]
    pub(crate) trace: Vec<String>,
    /// Conditions under which it holds (auth, network, timing, ...).
    #[serde(default)]
    pub(crate) conditions: Vec<String>,
    /// How an attacker would use it.
    #[serde(default)]
    pub(crate) execution: Option<Execution>,
    /// ATT&CK technique id, e.g. `T1078`.
    #[serde(default)]
    pub(crate) mitre: Option<String>,
    /// 0..1: how strongly the evidence supports the finding.
    #[serde(default)]
    pub(crate) confidence: Option<f64>,
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
    /// When the run started, for the spend footer.
    pub(crate) started: std::time::Instant,
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
            started: std::time::Instant::now(),
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
pub struct Engine {
    /// The bridge client.
    pub(crate) llm: Client,
    /// The runner client.
    pub(crate) runner: Sandbox,
    /// Endpoints and limits.
    pub(crate) config: Config,
}

impl Engine {
    /// Build a pipeline from a configuration.
    #[must_use]
    pub fn new(config: Config) -> Self {
        let llm = Client::local()
            .with_base_url(config.bridge.clone())
            .with_key(config.bridge_key.clone());
        let runner = Sandbox::local().with_base_url(config.runner.clone());
        Self {
            llm,
            runner,
            config,
        }
    }

    /// Run one agent's phases under a fresh run id, calling the option's
    /// checkpoint after each phase with the current ledger, target and agent.
    ///
    /// # Errors
    /// Fails if the runner preflight fails, the bridge serves no model, or
    /// any phase fails.
    pub fn run_checkpointed(
        &self,
        agent: &Agent,
        target: &str,
        opts: &mut RunOptions<'_>,
    ) -> Result<Assessment, String> {
        let run_id = new_run_id(target);
        self.run_identified(agent, target, &run_id, opts)
    }

    /// Run one agent's phases under a caller-chosen run id, calling the
    /// option's checkpoint after each phase. The autonomy trigger records the
    /// id in the run index before its tail starts, so the report and the index
    /// name the same run.
    ///
    /// # Errors
    /// Fails if the runner preflight fails, the bridge serves no model, or
    /// any phase fails.
    pub fn run_identified(
        &self,
        agent: &Agent,
        target: &str,
        run_id: &str,
        opts: &mut RunOptions<'_>,
    ) -> Result<Assessment, String> {
        let (ctx, mut state) = self.prepare(agent, target, opts)?;
        for phase in planned_phases(agent) {
            self.step(phase, &ctx, &mut state)?;
            if let Some(cp) = opts.checkpoint.as_mut() {
                cp(&state.ledger, target, agent.name);
            }
        }
        self.finish(run_id, agent, &ctx, &mut state)
    }

    /// Resolve the model and build the starting state of one run; the caller
    /// steps the phases so it can observe and steer between them.
    ///
    /// # Errors
    /// Fails if the runner preflight fails or the bridge serves no model.
    pub(crate) fn prepare(
        &self,
        agent: &Agent,
        target: &str,
        opts: &RunOptions<'_>,
    ) -> Result<(ModelCtx, RunState), String> {
        self.runner
            .health()
            .map_err(|error| format!("runner preflight failed: {error}"))?;
        let ctx = ModelCtx {
            system: system_prompt(agent, opts.pattern),
            model: self.resolve_model(opts.model.as_deref())?,
        };
        Ok((ctx, RunState::new(target)))
    }

    /// Execute one phase of a prepared run.
    ///
    /// # Errors
    /// Fails with the phase's own error.
    pub(crate) fn step(
        &self,
        phase: PhaseKind,
        ctx: &ModelCtx,
        state: &mut RunState,
    ) -> Result<(), String> {
        self.execute(phase, ctx, state)
    }

    /// Assemble and retain the report of a prepared run.
    ///
    /// # Errors
    /// Fails if the ledger or the report cannot be written.
    pub(crate) fn finish(
        &self,
        run_id: &str,
        agent: &Agent,
        ctx: &ModelCtx,
        state: &mut RunState,
    ) -> Result<Assessment, String> {
        self.assemble(run_id, agent, ctx, state)
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
                state.recon = self.phase_recon(&state.target, &mut state.ledger)?;
            }
            PhaseKind::Model => {
                state.target_model = self.phase_target_model(
                    ctx,
                    &state.target,
                    &state.recon,
                    &mut state.ledger,
                )?;
            }
            PhaseKind::Hypotheses => {
                state.hypotheses =
                    self.phase_hypotheses(ctx, &state.target_model, &mut state.ledger);
            }
            PhaseKind::Verify => self.phase_verify(ctx, state),
            PhaseKind::Deepen => self.phase_deepen(ctx, state),
            PhaseKind::Threat => {
                state.threats =
                    self.phase_threat(ctx, &state.target_model, &mut state.ledger);
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
    ) -> Result<Assessment, String> {
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
        let mut findings = std::mem::take(&mut state.findings);
        findings.sort_by(|a, b| a.fingerprint.cmp(&b.fingerprint));
        let mut spend = state.ledger.spend();
        spend.duration_secs = state.started.elapsed().as_secs();
        let report = Assessment {
            schema: SCHEMA.into(),
            run_id: run_id.into(),
            agent: agent.name.into(),
            target: state.target.clone(),
            model: ctx.model.clone(),
            phases: state.ledger.phases().to_vec(),
            findings,
            unknown,
            refuted: std::mem::take(&mut state.refuted),
            threats: std::mem::take(&mut state.threats),
            runtime_mechanics: mechanics,
            spend: Some(spend),
            summary,
        };
        state.ledger.save(run_id, &state.target, agent.name, &ctx.model)?;
        crate::evidence::save_report(run_id, &report)?;
        Ok(report)
    }

    /// Resolve the run's model: explicit override, else the bridge's first.
    fn resolve_model(&self, override_model: Option<&str>) -> Result<String, String> {
        if let Some(model) = override_model {
            return Ok(model.to_string());
        }
        self.llm.available_models().and_then(|models| {
            let preferred = models
                .iter()
                .find(|m| **m == self.config.default_model)
                .cloned();
            preferred
                .or_else(|| models.into_iter().next())
                .ok_or_else(|| "bridge /models returned no models".to_string())
        })
    }
}

/// Run one assessment with a configuration from the environment.
///
/// # Errors
/// Fails if the target is empty or the pipeline run fails.
pub fn assess(params: &AssessParams) -> Result<Assessment, String> {
    let run_id = new_run_id(&params.target);
    assess_identified(params, &run_id)
}

/// Run one assessment under a caller-chosen run id; the autonomy trigger
/// records that id in the run index before its tail starts.
///
/// # Errors
/// Fails if the target is empty or the pipeline run fails.
pub fn assess_identified(params: &AssessParams, run_id: &str) -> Result<Assessment, String> {
    let (engine, agent, mut opts) = assessment_setup(params)?;
    engine.run_identified(agent, &params.target, run_id, &mut opts)
}

/// Resolve the shared target validation, environment and per-run options.
///
/// # Errors
/// Returns an error when the assessment target is blank.
pub(crate) fn assessment_setup(
    params: &AssessParams,
) -> Result<(Engine, &'static Agent, RunOptions<'static>), String> {
    if params.target.trim().is_empty() {
        return Err("target is required".into());
    }
    let mut config = Config::from_env();
    if let Some(n) = params.max_hypotheses
        && n > 0
    {
        config.max_hypotheses = n;
    }
    let agent = crate::agents::resolve_agent(params.agent.as_deref());
    let opts = RunOptions {
        model: params.model.clone(),
        pattern: Pattern::parse(params.pattern.as_deref().unwrap_or_default()),
        checkpoint: None,
    };
    Ok((Engine::new(config), agent, opts))
}

/// Retrieve the retained evidence ledger for a finished run.
///
/// # Errors
/// Fails if no ledger was retained for the run id.
pub fn evidence_for(run_id: &str) -> Result<String, String> {
    crate::evidence::load(run_id)
}

/// Resolve a model for a run: an explicit override wins, else the configured
/// default when the bridge serves it, else the bridge's first model.
///
/// # Errors
/// Fails if the bridge serves no models.
pub fn resolve_model(
    client: &crate::llm::Client,
    default_model: &str,
    override_model: Option<&str>,
) -> Result<String, String> {
    if let Some(model) = override_model {
        return Ok(model.to_string());
    }
    let models = client.available_models()?;
    let preferred = models.iter().find(|m| **m == default_model).cloned();
    preferred
        .or_else(|| models.into_iter().next())
        .ok_or_else(|| "bridge /models returned no models".to_string())
}

/// The system prompt for one agent: the unified prompt, the role, and the
/// pattern guidance in that order.
fn system_prompt(agent: &Agent, pattern: Pattern) -> String {
    format!(
        "{}\n\nAGENT ROLE:\n{}\n\n{}",
        methodology::SYSTEM_PROMPT,
        agent.role,
        pattern.guidance()
    )
}

/// A filename-safe, collision-free run id: `run-<millis>-<seq>-<slug>`.
///
/// The sequence is a per-process counter, so two runs started inside one
/// millisecond, or for two targets sharing the first 24 slug characters,
/// still get distinct ids.
pub(crate) fn new_run_id(target: &str) -> String {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let seq = SEQUENCE.fetch_add(1, Ordering::SeqCst);
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
    format!("run-{millis}-{seq}-{slug}")
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
    let verdict = if v.level == "CONFIRMED" {
        Verdict::Confirmed
    } else {
        Verdict::NeedsValidation
    };
    let title = truncate(&v.hypothesis.claim, 90).to_string();
    Finding {
        id: format!("F-{id:03}"),
        fingerprint: fingerprint(target, &v.hypothesis.class, &title),
        title,
        class: v.hypothesis.class.clone(),
        verdict,
        severity: "info".into(),
        likelihood: String::new(),
        confidence: if v.level == "CONFIRMED" { 0.5 } else { 0.3 },
        trace: Vec::new(),
        conditions: Vec::new(),
        execution: None,
        mitre: None,
        poc: None,
        evidence_level: v.level.clone(),
        escalation: String::new(),
        evidence: Evidence {
            command: v.command.clone(),
            output: v.output.clone(),
            id: v.evidence_id.clone(),
        },
        impact: v.reasoning.clone(),
        fix: String::new(),
        target: target.into(),
        claimed_root_cause: Some(v.hypothesis.claim.clone()),
        blockers: if verdict == Verdict::NeedsValidation {
            v.unknown.clone()
        } else {
            Vec::new()
        },
        validation_plan: None,
        reason: None,
    }
}

/// The stable fingerprint of a finding: target, class and title slug.
///
/// # Errors
/// Does not fail; the slug is total over any input.
pub(crate) fn fingerprint(target: &str, class: &str, title: &str) -> String {
    let slug = |s: &str| {
        s.chars()
            .take(40)
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect::<String>()
    };
    format!("{}::{}::{}", slug(target), slug(class), slug(title))
}

/// Deterministic fallback summary from the report data.
pub(crate) fn default_summary_for(report_data: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(report_data).unwrap_or_default();
    let findings = value
        .get("findings")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    let refuted = value
        .get("refuted")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
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
.filter(|f| f.verdict == Verdict::Confirmed)
.count(),
refuted.len()
)
}

/// Pull the outermost JSON object out of a model response.
pub(crate) fn extract_json(raw: &str) -> String {
    let start = raw.find('{').unwrap_or(0);
    let end = raw.rfind('}').map_or(raw.len(), |i| i + 1);
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

    /// Two runs started in the same second get different ids.
    #[test]
    fn run_ids_differ_within_one_second() {
        let first = new_run_id("example.com");
        let second = new_run_id("example.com");
        assert_ne!(first, second, "two runs in the same second must differ");
    }

    /// Two targets sharing the first 24 slug characters get different ids.
    #[test]
    fn run_ids_differ_for_targets_sharing_a_slug() {
        let first = new_run_id("https://example.com/aaaaaaaaaaaaaaaaaaaaaaa1");
        let second = new_run_id("https://example.com/aaaaaaaaaaaaaaaaaaaaaaa2");
        assert_ne!(first, second, "a shared slug prefix must not share an id");
    }

    /// Unknown agent names fall back to the default reviewer.
    #[test]
    fn unknown_agent_falls_back_to_the_default() {
        assert_eq!(
            crate::agents::resolve_agent(Some("nope")).name,
            "maestro.security"
        );
    }
}
