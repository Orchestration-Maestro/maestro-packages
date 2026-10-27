//! Tool names, descriptions and JSON schemas: one copy, used by both the
//! extension surface and the MCP surface.

use serde_json::json;

/// Name of the assessment tool.
pub const ASSESS: &str = "security_assess";
/// Name of the evidence tool.
pub const EVIDENCE: &str = "security_evidence";
/// Name of the workspace tool.
pub const WORKSPACE: &str = "security_workspace";
/// Name of the doctor tool.
pub const DOCTOR: &str = "security_doctor";
/// Name of the route tool.
pub const ROUTE: &str = "security_route";
/// Name of the report-page tool.
pub const REPORT_PAGE: &str = "security_report_page";
/// Name of the evidence-page tool.
pub const EVIDENCE_PAGE: &str = "security_evidence_page";
/// Name of the validate tool.
pub const VALIDATE: &str = "security_validate";
/// Name of the disclose tool.
pub const DISCLOSE: &str = "security_disclose";

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
untruncated outputs of every step) for a finished run, by run id, or by agent \
name (the newest retained run by that agent).";

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
        "properties": {
            "run_id": {
                "type": "string",
                "description": "Run id from a finished assessment report"
            },
            "agent": {
                "type": "string",
                "description": "Agent name; resolves the newest retained run by that agent"
            }
        },
        "anyOf": [{ "required": ["run_id"] }, { "required": ["agent"] }]
    })
}

/// Description of the workspace tool.
pub const WORKSPACE_DESCRIPTION: &str = "Manage named workspaces: start and resume \
assessments with a persistent run id, read status and logs, list workspaces, \
stop a run, and keep the workspace card deck and credential store. Actions: \
start, resume, status, logs, list, stop, cards:add, cards:list, cards:update, \
creds:add, creds:list.";

/// Description of the doctor tool.
pub const DOCTOR_DESCRIPTION: &str = "Check the health of the security box: runner \
reachability, bridge reachability, model availability, a live one-token \
inference probe on the default model, sandbox tooling, and host hardware. \
Each check carries a classification (healthy, config_drift, \
missing_analysis_engine, unsupported_host) and a remediation line.";

/// Description of the route tool.
pub const ROUTE_DESCRIPTION: &str = "Route a task hint to the domain that owns it \
(web, network, binary, source, cloud, identity, ctf), using the domain list \
as data. Use it before dispatching a specialist task.";

/// Description of the report-page tool.
pub const REPORT_PAGE_DESCRIPTION: &str = "Render the retained report of a finished \
run as a self-contained Forge-branded HTML page. The returned HTML is a \
Maestro artifact: drop it in as a .html artifact and it becomes a live \
webpage.";

/// Description of the evidence-page tool.
pub const EVIDENCE_PAGE_DESCRIPTION: &str = "Render the retained evidence ledger of a \
finished run as a self-contained Forge-branded HTML page, one row per \
ledgered step.";

/// Description of the validate tool.
pub const VALIDATE_DESCRIPTION: &str = "Validate the retained report of a finished \
run against the schema and semantic invariants: fingerprints unique and \
sorted, per-verdict evidence rules, severity bounds. Returns the violations, \
empty when the report is sound.";

/// JSON schema for the workspace tool parameters.
#[must_use]
pub fn workspace_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "required": ["action"],
        "properties": {
            "action": {
                "type": "string",
                "enum": ["start", "resume", "status", "logs", "list", "stop",
                         "cards:add", "cards:list", "cards:update",
                         "creds:add", "creds:list"],
                "description": "The workspace action"
            },
            "name": { "type": "string", "description": "Workspace name" },
            "target": { "type": "string", "description": "Target for start/resume" },
            "agent": { "type": "string", "description": "Agent role for start/resume" },
            "model": { "type": "string", "description": "Model override for start/resume" },
            "pattern": {
                "type": "string",
                "enum": ["red_team", "blue_team", "bug_bounter", "compliance",
                         "continuous_ops", "dfir"],
                "description": "Operational pattern for start/resume"
            },
            "payload": {
                "type": "object",
                "description": "Card or credential payload for the card and credential actions"
            }
        }
    })
}

/// JSON schema for the route tool parameters.
#[must_use]
pub fn route_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "required": ["task"],
        "properties": {
            "task": { "type": "string", "description": "The task hint to route" }
        }
    })
}

/// JSON schema for the page tools: one run id.
#[must_use]
pub fn page_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "required": ["run_id"],
        "properties": {
            "run_id": { "type": "string", "description": "Run id of the finished run" }
        }
    })
}

// ---------------------------------------------------------------------------
// Shared runners: one implementation, used by both surfaces.
// ---------------------------------------------------------------------------

/// Run one assessment; the report JSON is the result.
///
/// # Errors
/// Fails if the arguments are malformed or the underlying capability fails.
pub fn run_assess(arguments: &serde_json::Value) -> Result<String, String> {
    let params: crate::pipeline::AssessParams =
        serde_json::from_value(arguments.clone()).map_err(|e| e.to_string())?;
    let report =
        crate::pipeline::assess(&params).map_err(|error| format!("assessment failed: {error}"))?;
    serde_json::to_string(&report).map_err(|e| e.to_string())
}

/// The retained evidence ledger for one finished run.
///
/// # Errors
/// Fails if the arguments are malformed or the underlying capability fails.
pub fn run_evidence(arguments: &serde_json::Value) -> Result<String, String> {
    let run_id = match arguments.get("run_id").and_then(serde_json::Value::as_str) {
        Some(id) => id.to_string(),
        None => latest_run_for_agent(arguments)?,
    };
    crate::pipeline::evidence_for(&run_id).map_err(|e| format!("evidence lookup failed: {e}"))
}

/// Resolve the newest retained run by an agent name.
///
/// # Errors
/// Fails if no agent is given or the agent has no retained runs.
fn latest_run_for_agent(arguments: &serde_json::Value) -> Result<String, String> {
    let agent = arguments
        .get("agent")
        .and_then(serde_json::Value::as_str)
        .ok_or("run_id or agent is required")?;
    crate::store::Store
        .list_runs(0)?
        .into_iter()
        .find(|run| run.agent == agent)
        .map(|run| run.run_id)
        .ok_or_else(|| format!("no retained runs by agent {agent}"))
}

/// One workspace action; the result JSON is the output.
///
/// # Errors
/// Fails if the arguments are malformed or the underlying capability fails.
pub fn run_workspace(arguments: &serde_json::Value) -> Result<String, String> {
    let params: crate::workspace::Order =
        serde_json::from_value(arguments.clone()).map_err(|e| e.to_string())?;
    let result = crate::workspace::dispatch(&params)
        .map_err(|error| format!("workspace failed: {error}"))?;
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

/// The doctor report; the JSON is the output.
///
/// # Errors
/// Fails if the arguments are malformed or the underlying capability fails.
pub fn run_doctor() -> Result<String, String> {
    let report = crate::doctor::doctor().map_err(|e| format!("doctor failed: {e}"))?;
    serde_json::to_string(&report).map_err(|e| e.to_string())
}

/// Route one task hint; the domain JSON is the output.
///
/// # Errors
/// Fails if the arguments are malformed or the underlying capability fails.
pub fn run_route(arguments: &serde_json::Value) -> Result<String, String> {
    let task = arguments
        .get("task")
        .and_then(serde_json::Value::as_str)
        .ok_or("task is required")?;
    let router = crate::router::Router::default_();
    match router.route(task) {
        Some(domain) => serde_json::to_string(domain).map_err(|e| e.to_string()),
        None => Ok(format!("no domain matched: {task}")),
    }
}

/// The Forge report page for one finished run.
///
/// # Errors
/// Fails if the arguments are malformed or the underlying capability fails.
pub fn run_report_page(arguments: &serde_json::Value) -> Result<String, String> {
    let run_id = arguments
        .get("run_id")
        .and_then(serde_json::Value::as_str)
        .ok_or("run_id is required")?;
    let report =
        crate::evidence::load_report(run_id).map_err(|e| format!("report lookup failed: {e}"))?;
    let page = crate::webui::render(&report);
    Ok(page.html)
}

/// The Forge evidence page for one finished run.
///
/// # Errors
/// Fails if the arguments are malformed or the underlying capability fails.
pub fn run_evidence_page(arguments: &serde_json::Value) -> Result<String, String> {
    let run_id = arguments
        .get("run_id")
        .and_then(serde_json::Value::as_str)
        .ok_or("run_id is required")?;
    let text = crate::evidence::load(run_id).map_err(|e| format!("evidence lookup failed: {e}"))?;
    let doc: crate::evidence::LedgerDoc =
        serde_json::from_str(&text).map_err(|e| format!("ledger {run_id}: {e}"))?;
    crate::webui::render_evidence(&doc).map(|page| page.html)
}

/// Description of the disclose tool.
pub const DISCLOSE_DESCRIPTION: &str = "Turn findings into disclosure artifacts: \n\"brief\" drafts a coordinated disclosure (severity, recipient, timeline, \nready-to-send draft) for the given findings; \"sarif\" exports a finished run's \nreport as a SARIF 2.1.0 document.";

/// JSON schema for the disclose tool parameters.
#[must_use]
pub fn disclose_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "required": ["action"],
        "properties": {
            "action": {
                "type": "string",
                "enum": ["brief", "sarif"],
                "description": "brief: draft a coordinated disclosure; sarif: export a run as SARIF"
            },
            "target": { "type": "string", "description": "The target (brief)" },
            "findings": {
                "type": "array",
                "items": { "type": "string" },
                "description": "The findings as text (brief)"
            },
            "run_id": { "type": "string", "description": "Run id to export (sarif)" },
            "model": {
                "type": "string",
                "description": "Optional model override (brief)"
            }
        }
    })
}

/// Validate the retained report of one finished run.
///
/// # Errors
/// Fails if the arguments are malformed or the underlying capability fails.
pub fn run_validate(arguments: &serde_json::Value) -> Result<String, String> {
    let run_id = arguments
        .get("run_id")
        .and_then(serde_json::Value::as_str)
        .ok_or("run_id is required")?;
    let report =
        crate::evidence::load_report(run_id).map_err(|e| format!("report lookup failed: {e}"))?;
    let verdict = crate::validate::validate_assessment(&report);
    serde_json::to_string(&verdict).map_err(|e| e.to_string())
}

/// Run the disclosure tool: draft a coordinated disclosure or export SARIF.
///
/// # Errors
/// Fails if the action is unknown, the arguments are malformed, or the
/// underlying capability fails.
pub fn run_disclose(arguments: &serde_json::Value) -> Result<String, String> {
    let action = arguments
        .get("action")
        .and_then(serde_json::Value::as_str)
        .ok_or("action is required")?;
    match action {
        "brief" => {
            let params: crate::disclose::Brief =
                serde_json::from_value(arguments.clone()).map_err(|e| e.to_string())?;
            let disclosure = crate::disclose::disclose(&params)
                .map_err(|error| format!("disclosure failed: {error}"))?;
            serde_json::to_string(&disclosure).map_err(|e| e.to_string())
        }
        "sarif" => {
            let run_id = arguments
                .get("run_id")
                .and_then(serde_json::Value::as_str)
                .ok_or("run_id is required for sarif")?;
            let report = crate::evidence::load_report(run_id)
                .map_err(|e| format!("report lookup failed: {e}"))?;
            Ok(crate::disclose::sarif(&report))
        }
        other => Err(format!("unknown disclose action: {other}")),
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the shared tool runners.

    use super::run_evidence;

    /// Evidence with neither run id nor agent is a clear error.
    #[test]
    fn evidence_requires_run_id_or_agent() {
        let err = run_evidence(&serde_json::json!({})).unwrap_err();
        assert!(
            err.contains("run_id or agent is required"),
            "unexpected: {err}"
        );
    }

    /// Evidence for an agent with no retained runs is a clear error.
    #[test]
    fn evidence_agent_without_runs_is_an_error() {
        let err = run_evidence(&serde_json::json!({
            "agent": "test-agent-never-run-xyz"
        }))
        .unwrap_err();
        assert!(err.contains("no retained runs"), "unexpected: {err}");
    }
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod behavior_tests;
