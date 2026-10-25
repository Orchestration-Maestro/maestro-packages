//! The evidence ledger: sequential ids for every executed step, plus
//! retention of the full (untruncated) outputs in one ledger file per run.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::report::Phase;

/// Schema tag for the retained ledger document.
pub const SCHEMA: &str = "maestro-security/evidence/1";

/// One retained step of a run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    /// Sequential evidence id, e.g. `ev-0001`.
    pub id: String,
    /// Phase name: recon, target-model, hypotheses, verify, deepen, threat, report.
    pub phase: String,
    /// exec | llm | note.
    pub kind: String,
    /// The executed command (exec entries).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// The prompt sent (llm entries).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// The full output, untruncated.
    pub output: String,
    /// The model that produced it (llm entries).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Output tokens reported by the bridge (llm entries).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_out: Option<u64>,
    /// The process exit code (exec entries).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
}

/// The ledger document retained for one run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerDoc {
    /// Schema tag.
    pub schema: String,
    /// Run id.
    pub run_id: String,
    /// Assessed target.
    pub target: String,
    /// Agent that ran.
    pub agent: String,
    /// Model used (resolved at run time).
    pub model: String,
    /// Every step, in execution order.
    pub entries: Vec<Entry>,
}

/// One model call, as recorded in the ledger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCall {
    /// The prompt sent to the model.
    pub prompt: String,
    /// The model's full output, untruncated.
    pub output: String,
    /// The model that produced it.
    pub model: String,
    /// Output tokens reported by the bridge; 0 when it reports none.
    #[serde(default)]
    pub tokens_out: u64,
}

/// The in-run ledger: assigns sequential evidence ids and accumulates
/// entries for retention.
pub struct Ledger {
    /// The next sequential evidence number.
    next: u32,
    /// The phases run so far, in order.
    phases: Vec<crate::report::Phase>,
    /// Every retained entry.
    entries: Vec<Entry>,
}

impl Default for Ledger {
    fn default() -> Self {
        Self::new()
    }
}

impl Ledger {
    /// An empty ledger; the first evidence id is `ev-0001`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            next: 1,
            phases: Vec::new(),
            entries: Vec::new(),
        }
    }

    /// Record one executed command; returns its evidence id.
    pub fn record_exec(
        &mut self,
        phase: &str,
        command: &str,
        output: &str,
        exit_code: i32,
    ) -> String {
        let id = self.next_id();
        self.phases.push(Phase {
            name: phase.into(),
            evidence_id: id.clone(),
            note: format!("{command} (exit {exit_code})"),
        });
        self.entries.push(Entry {
            id: id.clone(),
            phase: phase.into(),
            kind: "exec".into(),
            command: Some(command.into()),
            prompt: None,
            output: output.into(),
            model: None,
            tokens_out: None,
            exit_code: Some(exit_code),
        });
        id
    }

    /// Record one model call; returns its evidence id.
    pub fn record_llm(&mut self, phase: &str, note: &str, call: &LlmCall) -> String {
        let id = self.next_id();
        self.phases.push(Phase {
            name: phase.into(),
            evidence_id: id.clone(),
            note: note.into(),
        });
        self.entries.push(Entry {
            id: id.clone(),
            phase: phase.into(),
            kind: "llm".into(),
            command: None,
            prompt: Some(call.prompt.clone()),
            output: call.output.clone(),
            model: Some(call.model.clone()),
            tokens_out: Some(call.tokens_out),
            exit_code: None,
        });
        id
    }

    /// Record a step with no external output; returns its evidence id.
    pub fn record_note(&mut self, phase: &str, note: &str) -> String {
        let id = self.next_id();
        self.phases.push(Phase {
            name: phase.into(),
            evidence_id: id.clone(),
            note: note.into(),
        });
        self.entries.push(Entry {
            id: id.clone(),
            phase: phase.into(),
            kind: "note".into(),
            command: None,
            prompt: None,
            output: note.into(),
            model: None,
            tokens_out: None,
            exit_code: None,
        });
        id
    }

    /// What the run spent: LLM calls and output tokens across the ledger.
    #[must_use]
    pub fn spend(&self) -> crate::report::RunSpend {
        let llm_calls = self.entries.iter().filter(|e| e.kind == "llm").count();
        let tokens_out = self
            .entries
            .iter()
            .filter_map(|e| e.tokens_out)
            .sum::<u64>();
        crate::report::RunSpend {
            llm_calls,
            tokens_out,
            ..Default::default()
        }
    }

    /// The phase log, in execution order.
    #[must_use]
    pub fn phases(&self) -> &[Phase] {
        &self.phases
    }

    /// Retain the full ledger for the run; returns the file path.
    ///
    /// # Errors
    /// Fails if the evidence directory cannot be created or the file cannot
    /// be written.
    pub fn save(
        &self,
        run_id: &str,
        target: &str,
        agent: &str,
        model: &str,
    ) -> Result<PathBuf, String> {
        let dir = home()?;
        std::fs::create_dir_all(&dir).map_err(|error| format!("evidence dir: {error}"))?;
        let path = dir.join(format!("{run_id}.json"));
        let doc = LedgerDoc {
            schema: SCHEMA.into(),
            run_id: run_id.into(),
            target: target.into(),
            agent: agent.into(),
            model: model.into(),
            entries: self.entries.clone(),
        };
        let json =
            serde_json::to_string_pretty(&doc).map_err(|error| format!("evidence: {error}"))?;
        std::fs::write(&path, json).map_err(|error| format!("evidence write: {error}"))?;
        Ok(path)
    }

    /// The next sequential evidence id, then advance the counter.
    fn next_id(&mut self) -> String {
        let id = format!("ev-{:04}", self.next);
        self.next += 1;
        id
    }
}

/// The base state directory: `$MAESTRO_SECURITY_HOME/.maestro-security` or
/// `$HOME/.maestro-security`.
///
/// # Errors
/// Fails if neither environment variable is set or `$HOME` is missing.
pub fn base_dir() -> Result<PathBuf, String> {
    let base = std::env::var("MAESTRO_SECURITY_HOME")
        .ok()
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .ok_or("set MAESTRO_SECURITY_HOME or HOME")?;
    Ok(base.join(".maestro-security"))
}

/// The evidence ledger directory under the base state directory.
fn home() -> Result<PathBuf, String> {
    Ok(base_dir()?.join("evidence"))
}

/// Load a retained ledger by run id.
///
/// # Errors
/// Fails if no ledger was retained for the run id.
pub fn load(run_id: &str) -> Result<String, String> {
    let path = home()?.join(format!("{run_id}.json"));
    std::fs::read_to_string(&path)
        .map_err(|_| format!("no evidence ledger for run {run_id} at {}", path.display()))
}

/// The report directory under the base state directory.
fn reports() -> Result<PathBuf, String> {
    Ok(base_dir()?.join("reports"))
}

/// Retain the finished report for one run, next to its ledger.
///
/// # Errors
/// Fails if the state directory cannot be created or the write fails.
pub fn save_report(run_id: &str, report: &crate::report::Assessment) -> Result<PathBuf, String> {
    let dir = reports()?;
    std::fs::create_dir_all(&dir).map_err(|error| format!("reports dir: {error}"))?;
    let path = dir.join(format!("{run_id}.json"));
    let json =
        serde_json::to_string_pretty(report).map_err(|error| format!("report: {error}"))?;
    std::fs::write(&path, json).map_err(|error| format!("report write: {error}"))?;
    Ok(path)
}

/// Load a retained report by run id.
///
/// # Errors
/// Fails if no report was retained for the run id.
pub fn load_report(run_id: &str) -> Result<crate::report::Assessment, String> {
    let path = reports()?.join(format!("{run_id}.json"));
    let text = std::fs::read_to_string(&path)
        .map_err(|_| format!("no report for run {run_id} at {}", path.display()))?;
    serde_json::from_str(&text).map_err(|error| format!("report {run_id}: {error}"))
}
