//! Run and workspace persistence: the file-backed state that makes runs
//! observable, addressable, resumable and searchable across tool calls.
//!
//! Everything lives under the shared base state directory (see
//! [`crate::evidence::base_dir`]) so the evidence ledger, the run index and
//! the workspace table stay together on disk.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::evidence;
use crate::report::Assessment;

/// Lifecycle status of a run or a workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Started, not finished.
    Running,
    /// Finished successfully.
    Completed,
    /// Stopped before completion.
    Stopped,
    /// Failed.
    Failed,
}

/// One recorded run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    /// Run id from the report.
    pub run_id: String,
    /// Assessed target.
    pub target: String,
    /// Agent that ran.
    pub agent: String,
    /// Model used (resolved at run time).
    pub model: String,
    /// Lifecycle status.
    pub status: Status,
    /// Unix seconds the run started.
    pub started_at: u64,
    /// Unix seconds the run finished, if it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<u64>,
    /// Number of findings in the report.
    pub findings: usize,
    /// Number of hypotheses refuted.
    pub refuted: usize,
    /// One-line summary.
    pub summary: String,
    /// Owning workspace name, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
}

/// One named workspace: an addressable, resumable run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    /// Workspace name (addressable).
    pub name: String,
    /// Run id of the latest run.
    pub run_id: String,
    /// Assessed target.
    pub target: String,
    /// Agent role.
    pub agent: String,
    /// Model override, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Lifecycle status.
    pub status: Status,
    /// Unix seconds created.
    pub created_at: u64,
    /// Unix seconds last updated.
    pub updated_at: u64,
    /// Phases completed so far.
    pub phases_done: usize,
    /// Findings so far.
    pub findings: usize,
    /// Monotonic revision for optimistic concurrency; the store bumps it on
    /// every accepted update.
    #[serde(default)]
    pub revision: u64,
}

/// The file-backed store. Stateless: every method re-reads or re-writes the
/// on-disk files, so it is safe to clone and share.
#[derive(Clone, Copy)]
pub struct Store;

impl RunRecord {
    /// A record for a run that is starting now.
    #[must_use]
    pub fn starting(
        run_id: String,
        target: &str,
        agent: &str,
        model: &str,
        workspace: Option<&str>,
    ) -> Self {
        Self {
            run_id,
            target: target.to_string(),
            agent: agent.to_string(),
            model: model.to_string(),
            status: Status::Running,
            started_at: now_secs(),
            finished_at: None,
            findings: 0,
            refuted: 0,
            summary: "running".into(),
            workspace: workspace.map(str::to_string),
        }
    }

    /// Settle the record from its outcome: the finished stamp, the status,
    /// the model and the counts. `Ok` completes, `Err` fails with its cause.
    pub fn settle(&mut self, outcome: Result<&Assessment, &str>) {
        self.finished_at = Some(now_secs());
        match outcome {
            Ok(report) => {
                self.status = Status::Completed;
                self.model.clone_from(&report.model);
                self.findings = report.findings.len();
                self.refuted = report.refuted.len();
                self.summary.clone_from(&report.summary);
            }
            Err(error) => {
                self.status = Status::Failed;
                self.summary = error.to_string();
            }
        }
    }
}

impl Store {
    /// The run index file (`runs.jsonl`).
    fn runs_path() -> Result<PathBuf, String> {
        Ok(evidence::base_dir()?.join("runs.jsonl"))
    }

    /// The workspace table file (`workspaces.json`).
    fn workspaces_path() -> Result<PathBuf, String> {
        Ok(evidence::base_dir()?.join("workspaces.json"))
    }

    /// Insert or update a run record, keeping the index sorted by start time.
    ///
    /// # Errors
    /// Fails if the state directory cannot be created or the file cannot be
    /// written.
    pub fn upsert_run(&self, record: &RunRecord) -> Result<(), String> {
        let path = Self::runs_path()?;
        let mut runs = Self::read_runs(&path)?;
        if let Some(slot) = runs.iter_mut().find(|r| r.run_id == record.run_id) {
            *slot = record.clone();
        } else {
            runs.push(record.clone());
        }
        Self::write_jsonl(&path, &runs)
    }

    /// Recorded runs, most recent first, capped at `limit` (0 = all).
    ///
    /// # Errors
    /// Fails if the run index cannot be read or a line is malformed.
    pub fn list_runs(&self, limit: usize) -> Result<Vec<RunRecord>, String> {
        let runs = Self::read_runs(&Self::runs_path()?)?;
        let mut out: Vec<RunRecord> = runs.into_iter().rev().collect();
        if limit > 0 {
            out.truncate(limit);
        }
        Ok(out)
    }

    /// One run by id.
    ///
    /// # Errors
    /// Fails if the run index cannot be read or a line is malformed.
    pub fn get_run(&self, run_id: &str) -> Result<Option<RunRecord>, String> {
        let runs = Self::read_runs(&Self::runs_path()?)?;
        Ok(runs.into_iter().find(|r| r.run_id == run_id))
    }

    /// All workspaces, most recently updated first.
    ///
    /// # Errors
    /// Fails if the workspace table cannot be read or is malformed.
    pub fn list_workspaces(&self) -> Result<Vec<Workspace>, String> {
        let path = Self::workspaces_path()?;
        let Ok(text) = std::fs::read_to_string(path) else {
            return Ok(Vec::new());
        };
        let mut workspaces: Vec<Workspace> =
            serde_json::from_str(&text).map_err(|e| format!("workspaces.json: {e}"))?;
        workspaces.sort_by_key(|w| std::cmp::Reverse(w.updated_at));
        Ok(workspaces)
    }

    /// One workspace by name.
    ///
    /// # Errors
    /// Fails if the workspace table cannot be read or is malformed.
    pub fn get_workspace(&self, name: &str) -> Result<Option<Workspace>, String> {
        Ok(self.list_workspaces()?.into_iter().find(|w| w.name == name))
    }

    /// Insert or update one workspace with optimistic concurrency: the
    /// caller passes the revision it read (`base`); the store rejects the
    /// update when the stored revision differs, so concurrent updates never
    /// overwrite each other silently. New workspaces must use base 0.
    ///
    /// # Errors
    /// Fails on a revision conflict, or if the state directory cannot be
    /// created or the file cannot be written.
    pub fn upsert_workspace(&self, base: u64, workspace: &Workspace) -> Result<(), String> {
        let path = Self::workspaces_path()?;
        let mut workspaces = Self::read_workspaces(&path)?;
        apply_workspace_update(&mut workspaces, base, workspace)?;
        Self::write_workspaces(&path, &workspaces)
    }

    /// The per-workspace cards file.
    fn cards_path(name: &str) -> Result<PathBuf, String> {
        Ok(evidence::base_dir()?.join(format!("cards-{name}.json")))
    }

    /// The per-workspace credentials file.
    fn creds_path(name: &str) -> Result<PathBuf, String> {
        Ok(evidence::base_dir()?.join(format!("creds-{name}.json")))
    }

    /// The cards of one workspace; a missing file is an empty deck.
    ///
    /// # Errors
    /// Fails if the file exists but is malformed.
    pub fn load_cards(&self, name: &str) -> Result<Vec<Card>, String> {
        read_doc(&Self::cards_path(name)?)
    }

    /// Save the cards of one workspace.
    ///
    /// # Errors
    /// Fails if the state directory cannot be created or the file cannot
    /// be written.
    pub fn save_cards(&self, name: &str, cards: &[Card]) -> Result<(), String> {
        write_doc(&Self::cards_path(name)?, &cards.to_vec())
    }

    /// The credentials of one workspace; a missing file is an empty set.
    ///
    /// # Errors
    /// Fails if the file exists but is malformed.
    pub fn load_creds(&self, name: &str) -> Result<Vec<Credential>, String> {
        read_doc(&Self::creds_path(name)?)
    }

    /// Save the credentials of one workspace.
    ///
    /// # Errors
    /// Fails if the state directory cannot be created or the file cannot
    /// be written.
    pub fn save_creds(&self, name: &str, creds: &[Credential]) -> Result<(), String> {
        write_doc(&Self::creds_path(name)?, &creds.to_vec())
    }

    /// Read the run index; a missing file is an empty index.
    fn read_runs(path: &Path) -> Result<Vec<RunRecord>, String> {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            out.push(serde_json::from_str(line).map_err(|e| format!("runs.jsonl: {e}"))?);
        }
        Ok(out)
    }

    /// Read the workspace table; a missing file is an empty table.
    fn read_workspaces(path: &Path) -> Result<Vec<Workspace>, String> {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Ok(Vec::new());
        };
        serde_json::from_str(&text).map_err(|e| format!("workspaces.json: {e}"))
    }

    /// Write the run index as one JSON record per line.
    fn write_jsonl(path: &Path, runs: &[RunRecord]) -> Result<(), String> {
        ensure_parent(path)?;
        let mut text = String::new();
        for record in runs {
            text.push_str(&serde_json::to_string(record).map_err(|e| e.to_string())?);
            text.push('\n');
        }
        std::fs::write(path, text).map_err(|e| format!("runs write: {e}"))
    }

    /// Write the workspace table as one JSON array.
    fn write_workspaces(path: &Path, workspaces: &[Workspace]) -> Result<(), String> {
        ensure_parent(path)?;
        let text = serde_json::to_string_pretty(workspaces).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| format!("workspaces write: {e}"))
    }
}

impl Default for Store {
    fn default() -> Self {
        Self
    }
}

/// Create the parent directory of a path if it is missing.
///
/// # Errors
/// Fails if the parent is missing and cannot be created.
fn ensure_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("state dir: {e}"))?;
    }
    Ok(())
}

/// Unix seconds since the epoch, for record timestamps.
#[must_use]
pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The kind of one card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardKind {
    /// A candidate finding.
    Finding,
    /// A fact observed during the run.
    Observation,
    /// Context or background.
    Info,
}

/// The status of one card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardStatus {
    /// Open: not yet decided.
    Open,
    /// Confirmed: the proof stands.
    Confirmed,
    /// Refuted: the evidence contradicts it.
    Refuted,
    /// Dismissed: not worth pursuing.
    Dismissed,
}

/// One card in a workspace deck: a finding, an observation, or a note.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    /// Stable id, e.g. `c-001`.
    pub id: String,
    /// The kind of card.
    pub kind: CardKind,
    /// One-line title.
    pub title: String,
    /// The body.
    pub body: String,
    /// The current status.
    pub status: CardStatus,
    /// The proof that confirmed it; required for confirmed cards.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proof: Option<String>,
    /// Unix seconds of creation.
    pub created_at: u64,
}

impl Card {
    /// Confirm the card with its proof; only open cards confirm.
    pub fn confirm(&mut self, proof: &str) -> bool {
        if self.status != CardStatus::Open {
            return false;
        }
        self.status = CardStatus::Confirmed;
        self.proof = Some(proof.to_string());
        true
    }

    /// Refute the card; only open cards refute.
    pub fn refute(&mut self) -> bool {
        if self.status != CardStatus::Open {
            return false;
        }
        self.status = CardStatus::Refuted;
        true
    }

    /// Dismiss the card; only open cards dismiss.
    pub fn dismiss(&mut self) -> bool {
        if self.status != CardStatus::Open {
            return false;
        }
        self.status = CardStatus::Dismissed;
        true
    }

    /// A confirmed card must carry its proof.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.status != CardStatus::Confirmed || self.proof.is_some()
    }
}

/// One credential in the workspace store. The secret is referenced by name,
/// never written into reports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credential {
    /// Stable id, e.g. `k-001`.
    pub id: String,
    /// bearer | cookie | ssh.
    pub kind: String,
    /// The target the credential applies to.
    pub target: String,
    /// The name of the secret, not the secret itself.
    pub secret_ref: String,
    /// One-line note.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// Unix seconds of creation.
    pub created_at: u64,
}

/// Read one JSON document; a missing file is the empty value.
fn read_doc<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return serde_json::from_str("null").map_err(|e| e.to_string());
    };
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Write one JSON document, creating the parent directory.
///
/// # Errors
/// Fails if the parent is missing and cannot be created, or the write fails.
fn write_doc<T: serde::Serialize>(path: &Path, doc: &T) -> Result<(), String> {
    ensure_parent(path)?;
    let text = serde_json::to_string_pretty(doc).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| format!("write {}: {e}", path.display()))
}

/// Apply one optimistic-concurrency update to the in-memory list: replace
/// the named workspace when `base` matches its stored revision, or create
/// it when the name is new (base must be 0).
///
/// # Errors
/// Fails on a revision conflict or a non-zero base for a new workspace.
fn apply_workspace_update(
    workspaces: &mut Vec<Workspace>,
    base: u64,
    workspace: &Workspace,
) -> Result<(), String> {
    if let Some(slot) = workspaces.iter_mut().find(|w| w.name == workspace.name) {
        if slot.revision != base {
            return Err(format!(
                "workspace {}: revision conflict (stored {}, expected {base})",
                workspace.name, slot.revision
            ));
        }
        *slot = workspace.clone();
        slot.revision = base + 1;
    } else {
        if base != 0 {
            return Err(format!(
                "workspace {}: expected a new workspace (base 0), got base {base}",
                workspace.name
            ));
        }
        let mut created = workspace.clone();
        created.revision = base + 1;
        workspaces.push(created);
    }
    Ok(())
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
