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
    /// The process exit code (exec entries).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
}

/// The ledger document retained for one run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerDoc {
    /// Schema tag.
    pub schema: &'static str,
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
#[derive(Debug, Clone)]
pub struct LlmCall {
    /// The prompt sent to the model.
    pub prompt: String,
    /// The model's full output, untruncated.
    pub output: String,
    /// The model that produced it.
    pub model: String,
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
            exit_code: None,
        });
        id
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
            schema: SCHEMA,
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

/// The ledger directory: `$MAESTRO_SECURITY_HOME` or `$HOME/.maestro-security`.
fn home() -> Result<PathBuf, String> {
    let base = std::env::var("MAESTRO_SECURITY_HOME")
        .ok()
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .ok_or("set MAESTRO_SECURITY_HOME or HOME")?;
    Ok(base.join(".maestro-security").join("evidence"))
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
