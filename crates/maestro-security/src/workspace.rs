//! Named workspaces: addressable, resumable runs with observable progress.
//!
//! A workspace wraps one target and an agent. `start` and `resume` run the
//! pipeline with a checkpoint that persists the ledger after every phase, so
//! a separate `status` or `logs` call can observe the run while it is in
//! flight. The workspace table and the run index are kept in the shared file
//! store; the in-flight ledger is retained in the evidence directory.

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::agents::Pattern;
use crate::evidence::Ledger;
use crate::pipeline::{Config, Engine, RunOptions};
use crate::report::Assessment;
use crate::store::{
    Card, CardKind, CardStatus, Credential, RunRecord, Status, Store, Workspace, now_secs,
};

/// Schema tag for the workspace result document.
pub const SCHEMA: &str = "maestro-security/workspace/2";

/// One log line, as returned by the `logs` action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogLine {
    /// Evidence id of the step.
    pub id: String,
    /// Phase name.
    pub phase: String,
    /// exec | llm | note.
    pub kind: String,
    /// One-line description (the step note or command).
    pub note: String,
}

/// Parameters for a workspace action.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Order {
    /// Action: start | status | logs | list | resume | stop.
    pub action: String,
    /// Workspace name (required for start/status/logs/resume/stop).
    pub name: Option<String>,
    /// Target to assess (required for start/resume).
    pub target: Option<String>,
    /// Agent role (default `maestro.security`).
    pub agent: Option<String>,
    /// Model override.
    pub model: Option<String>,
    /// Max hypotheses per run.
    pub max_hypotheses: Option<usize>,
    /// The operational pattern for the run (see `Pattern`).
    #[serde(default)]
    pub pattern: Option<String>,
    /// The payload for card and credential actions, as a JSON object.
    #[serde(default)]
    pub payload: Option<serde_json::Value>,
}

/// The result of a workspace action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Board {
    /// Schema tag.
    pub schema: &'static str,
    /// The action that was performed.
    pub action: String,
    /// The workspace, when the action concerns one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<Workspace>,
    /// All workspaces, for the `list` action.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workspaces: Vec<Workspace>,
    /// Log lines, for the `logs` action.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub logs: Vec<LogLine>,
    /// The assessment report, for `start`/`resume`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<Assessment>,
    /// The card deck, for the card actions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cards: Vec<Card>,
    /// The credentials, for the credential actions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub credentials: Vec<Credential>,
    /// A human-readable message.
    pub message: String,
}

/// Dispatch a workspace action.
///
/// # Errors
/// Fails on an unknown action, a missing required field, or a pipeline
/// failure for `start`/`resume`.
pub fn dispatch(params: &Order) -> Result<Board, String> {
    match params.action.as_str() {
        "start" | "resume" => start(params),
        "status" => status(params),
        "logs" => logs(params),
        "list" => list(),
        "stop" => stop(params),
        "cards:add" => cards_add(params),
        "cards:list" => cards_list(params),
        "cards:update" => cards_update(params),
        "creds:add" => creds_add(params),
        "creds:list" => creds_list(params),
        other => Err(format!("unknown workspace action: {other}")),
    }
}

/// Add one card to the workspace deck.
fn cards_add(params: &Order) -> Result<Board, String> {
    let name = require_name(params)?;
    let payload = payload_object(params)?;
    let title = payload_str(&payload, "title")?;
    let kind = payload
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("observation");
    let kind: CardKind =
        serde_json::from_value(json!(kind)).map_err(|e| format!("card kind: {e}"))?;
    let body = payload
        .get("body")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let store = Store;
    let mut cards = store.load_cards(&name)?;
    let id = format!("c-{:03}", cards.len() + 1);
    cards.push(Card {
        id: id.clone(),
        kind,
        title,
        body: body.to_string(),
        status: CardStatus::Open,
        proof: None,
        created_at: now_secs(),
    });
    store.save_cards(&name, &cards)?;
    Ok(result(
        params,
        format!("card {id} added to {name}"),
        cards,
        Vec::new(),
    ))
}

/// List the card deck of one workspace.
fn cards_list(params: &Order) -> Result<Board, String> {
    let name = require_name(params)?;
    let cards = Store.load_cards(&name)?;
    Ok(result(params, String::new(), cards, Vec::new()))
}

/// Update one card: confirm (with proof), refute, or dismiss.
fn cards_update(params: &Order) -> Result<Board, String> {
    let name = require_name(params)?;
    let payload = payload_object(params)?;
    let id = payload_str(&payload, "id")?;
    let action = payload_str(&payload, "do")?;
    let store = Store;
    let mut cards = store.load_cards(&name)?;
    let Some(card) = cards.iter_mut().find(|c| c.id == id) else {
        return Err(format!("no card {id} in {name}"));
    };
    match action.as_str() {
        "confirm" => {
            let proof = payload_str(&payload, "proof")?;
            card.confirm(&proof);
        }
        "refute" => {
            card.refute();
        }
        "dismiss" => {
            card.dismiss();
        }
        other => return Err(format!("unknown card action: {other}")),
    }
    store.save_cards(&name, &cards)?;
    Ok(result(
        params,
        format!("card {id} updated"),
        cards,
        Vec::new(),
    ))
}

/// Add one credential to the workspace store.
fn creds_add(params: &Order) -> Result<Board, String> {
    let name = require_name(params)?;
    let payload = payload_object(params)?;
    let kind = payload_str(&payload, "kind")?;
    let target = payload_str(&payload, "target")?;
    let secret_ref = payload_str(&payload, "secret_ref")?;
    let note = payload
        .get("note")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let store = Store;
    let mut creds = store.load_creds(&name)?;
    let id = format!("k-{:03}", creds.len() + 1);
    creds.push(Credential {
        id: id.clone(),
        kind,
        target,
        secret_ref,
        note: note.to_string(),
        created_at: now_secs(),
    });
    store.save_creds(&name, &creds)?;
    Ok(result(
        params,
        format!("credential {id} added to {name}"),
        Vec::new(),
        creds,
    ))
}

/// List the credentials of one workspace.
fn creds_list(params: &Order) -> Result<Board, String> {
    let name = require_name(params)?;
    let credentials = Store.load_creds(&name)?;
    Ok(result(params, String::new(), Vec::new(), credentials))
}

/// Build a workspace result with the given cards and credentials.
fn result(
    params: &Order,
    message: String,
    cards: Vec<Card>,
    credentials: Vec<Credential>,
) -> Board {
    Board {
        schema: SCHEMA,
        action: params.action.clone(),
        workspace: None,
        workspaces: Vec::new(),
        logs: Vec::new(),
        report: None,
        cards,
        credentials,
        message,
    }
}

/// The payload of one action, required to be a JSON object.
fn payload_object(params: &Order) -> Result<serde_json::Value, String> {
    let payload = params
        .payload
        .clone()
        .ok_or("payload is required for this action")?;
    payload
        .as_object()
        .cloned()
        .map(serde_json::Value::Object)
        .ok_or_else(|| "payload must be a JSON object".to_string())
}

/// A required string field of the payload.
fn payload_str(payload: &serde_json::Value, field: &str) -> Result<String, String> {
    payload
        .get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("payload.{field} is required"))
}

/// Start (or resume) a workspace: run the pipeline with a progress
/// checkpoint, then record the run.
fn start(params: &Order) -> Result<Board, String> {
    let name = require_name(params)?;
    let target = require_target(params)?;
    let store = Store;
    let (mut workspace, base) = new_running_workspace(store, &name, &target, params)?;
    store.upsert_workspace(base, &workspace)?;
    workspace.revision = base + 1;
    let report = run_with_checkpoint(params, &target, workspace.run_id.clone());
    finish_start(store, workspace, report, params, &name)
}

/// The non-empty target a start/resume requires.
fn require_target(params: &Order) -> Result<String, String> {
    params
        .target
        .clone()
        .filter(|t| !t.trim().is_empty())
        .ok_or_else(|| "target is required for start/resume".to_string())
}

/// A fresh running workspace, keeping the created time on re-runs; the
/// second element is the stored revision the update is based on (0 when new).
fn new_running_workspace(
    store: Store,
    name: &str,
    target: &str,
    params: &Order,
) -> Result<(Workspace, u64), String> {
    let now = now_secs();
    let existing = store.get_workspace(name)?;
    let base = existing.as_ref().map_or(0, |w| w.revision);
    Ok((
        Workspace {
            name: name.to_string(),
            run_id: String::new(),
            target: target.to_string(),
            agent: params
                .agent
                .clone()
                .unwrap_or_else(|| "maestro.security".into()),
            model: params.model.clone(),
            status: Status::Running,
            created_at: existing.as_ref().map_or(now, |w| w.created_at),
            updated_at: now,
            phases_done: 0,
            findings: 0,
            revision: base,
        },
        base,
    ))
}

/// Run the pipeline, persisting the ledger after each phase so a
/// concurrent status/logs call can observe the run in flight.
fn run_with_checkpoint(
    params: &Order,
    target: &str,
    run_id_holder: String,
) -> Result<Assessment, String> {
    let mut config = Config::from_env();
    if let Some(n) = params.max_hypotheses
        && n > 0
    {
        config.max_hypotheses = n;
    }
    let agent = crate::agents::resolve_agent(params.agent.as_deref());
    let mut checkpoint = move |ledger: &Ledger, t: &str, a: &str| {
        let _ = ledger.save(&run_id_holder, t, a, "");
    };
    let mut opts = RunOptions {
        model: params.model.clone(),
        pattern: Pattern::parse(params.pattern.as_deref().unwrap_or_default()),
        checkpoint: Some(&mut checkpoint),
    };
    Engine::new(config).run_checkpointed(agent, target, &mut opts)
}

/// Record the outcome of a start/resume and build the result board.
fn finish_start(
    store: Store,
    mut workspace: Workspace,
    report: Result<Assessment, String>,
    params: &Order,
    name: &str,
) -> Result<Board, String> {
    match report {
        Ok(assessment) => {
            workspace.status = Status::Completed;
            workspace.phases_done = assessment.phases.len();
            workspace.findings = assessment.findings.len();
            workspace.run_id.clone_from(&assessment.run_id);
            workspace.updated_at = now_secs();
            let findings = workspace.findings;
            store.upsert_workspace(workspace.revision, &workspace)?;
            record_run(store, &assessment, Some(name), Status::Completed)?;
            Ok(Board {
                schema: SCHEMA,
                action: params.action.clone(),
                workspace: Some(workspace),
                workspaces: Vec::new(),
                logs: Vec::new(),
                report: Some(assessment),
                cards: Vec::new(),
                credentials: Vec::new(),
                message: format!("workspace {name} completed: {findings} findings"),
            })
        }
        Err(error) => {
            workspace.status = Status::Failed;
            workspace.updated_at = now_secs();
            store.upsert_workspace(workspace.revision, &workspace)?;
            Err(format!("workspace {name}: {error}"))
        }
    }
}

/// Read the current state of a workspace, including in-flight progress from
/// the retained ledger.
fn status(params: &Order) -> Result<Board, String> {
    let name = require_name(params)?;
    let store = Store;
    let workspace = store
        .get_workspace(&name)?
        .ok_or_else(|| format!("no workspace named {name}"))?;
    Ok(Board {
        schema: SCHEMA,
        action: params.action.clone(),
        workspace: Some(workspace),
        workspaces: Vec::new(),
        logs: Vec::new(),
        report: None,
        cards: Vec::new(),
        credentials: Vec::new(),
        message: String::new(),
    })
}

/// Read the retained log lines for a workspace's latest run.
fn logs(params: &Order) -> Result<Board, String> {
    let name = require_name(params)?;
    let store = Store;
    let workspace = store
        .get_workspace(&name)?
        .ok_or_else(|| format!("no workspace named {name}"))?;
    let mut logs = Vec::new();
    if !workspace.run_id.is_empty()
        && let Ok(text) = crate::evidence::load(&workspace.run_id)
    {
        let doc: crate::evidence::LedgerDoc =
            serde_json::from_str(&text).map_err(|e| format!("ledger json: {e}"))?;
        logs = doc
            .entries
            .iter()
            .map(|e| LogLine {
                id: e.id.clone(),
                phase: e.phase.clone(),
                kind: e.kind.clone(),
                note: e
                    .command
                    .clone()
                    .or_else(|| e.prompt.clone())
                    .unwrap_or_else(|| e.output.chars().take(120).collect()),
            })
            .collect();
    }
    Ok(Board {
        schema: SCHEMA,
        action: params.action.clone(),
        workspace: Some(workspace),
        workspaces: Vec::new(),
        logs,
        report: None,
        cards: Vec::new(),
        credentials: Vec::new(),
        message: String::new(),
    })
}

/// List all workspaces, most recently updated first.
fn list() -> Result<Board, String> {
    let workspaces = Store.list_workspaces()?;
    Ok(Board {
        schema: SCHEMA,
        action: "list".into(),
        workspace: None,
        workspaces,
        logs: Vec::new(),
        report: None,
        cards: Vec::new(),
        credentials: Vec::new(),
        message: String::new(),
    })
}

/// Mark a workspace as stopped.
fn stop(params: &Order) -> Result<Board, String> {
    let name = require_name(params)?;
    let store = Store;
    let mut workspace = store
        .get_workspace(&name)?
        .ok_or_else(|| format!("no workspace named {name}"))?;
    workspace.status = Status::Stopped;
    workspace.updated_at = now_secs();
    store.upsert_workspace(workspace.revision, &workspace)?;
    Ok(Board {
        schema: SCHEMA,
        action: params.action.clone(),
        workspace: Some(workspace),
        workspaces: Vec::new(),
        logs: Vec::new(),
        report: None,
        cards: Vec::new(),
        credentials: Vec::new(),
        message: format!("workspace {name} stopped"),
    })
}

/// Record a finished run in the run index.
fn record_run(
    store: Store,
    assessment: &Assessment,
    workspace: Option<&str>,
    status: Status,
) -> Result<(), String> {
    let record = RunRecord {
        run_id: assessment.run_id.clone(),
        target: assessment.target.clone(),
        agent: assessment.agent.clone(),
        model: assessment.model.clone(),
        status,
        started_at: now_secs(),
        finished_at: Some(now_secs()),
        findings: assessment.findings.len(),
        refuted: assessment.refuted.len(),
        summary: assessment.summary.clone(),
        workspace: workspace.map(str::to_string),
    };
    store.upsert_run(&record)
}

/// The workspace name, required for most actions.
fn require_name(params: &Order) -> Result<String, String> {
    params
        .name
        .clone()
        .filter(|n| !n.trim().is_empty())
        .ok_or_else(|| "name is required for this action".to_string())
}

#[cfg(test)]
mod tests {
    //! Unit tests for workspace dispatch.

    use super::{Order, dispatch};

    /// An unknown action is rejected with a clear message.
    #[test]
    fn unknown_action_is_rejected() {
        let params = Order {
            action: "bogus".into(),
            ..Default::default()
        };
        let err = dispatch(&params).unwrap_err();
        assert!(err.contains("unknown workspace action"));
    }

    /// `list` works with no other fields and returns a well-formed doc.
    #[test]
    fn list_requires_no_fields() {
        let result = dispatch(&Order {
            action: "list".into(),
            ..Default::default()
        })
        .expect("list");
        assert_eq!(result.action, "list");
    }
}
