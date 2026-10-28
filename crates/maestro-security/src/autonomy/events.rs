//! The autonomy event contract: channel names, phase statuses and payloads.
//!
//! Every channel is a literal shared-bus channel; every payload names the run
//! so one subscriber can follow many concurrent runs.

use serde_json::{Value, json};

use crate::pipeline::PhaseKind;

/// Channel of the run-started event.
pub const STARTED: &str = "maestro.security.run.started";
/// Channel of a phase-transition event.
pub const PHASE: &str = "maestro.security.run.phase";
/// Channel of a finding event.
pub const FINDING: &str = "maestro.security.run.finding";
/// Channel of a refuted-hypothesis event.
pub const REFUTED: &str = "maestro.security.run.refuted";
/// Channel of a phase-checkpoint event.
pub const CHECKPOINT: &str = "maestro.security.run.checkpoint";
/// Channel of a steering delivery event.
pub const STEERED: &str = "maestro.security.run.steered";
/// Channel of a killed run.
pub const KILLED: &str = "maestro.security.run.killed";
/// Status of a killed run.
pub const STATUS_KILLED: &str = "killed";
/// Status of a run that has been opened.
pub const STATUS_STARTED: &str = "started";
/// Status of a phase that is executing.
pub const RUNNING: &str = "running";
/// Status of a phase that finished.
pub const COMPLETED: &str = "completed";
/// Status of a phase that failed.
pub const FAILED: &str = "failed";

/// The guidance delivered to a run's next phase.
#[must_use]
pub fn steered(run_id: &str, source: &str, message: &str) -> Value {
    json!({ "run_id": run_id, "source": source, "message": message })
}

/// A killed run and its last active phase.
#[must_use]
pub fn killed(run_id: &str, kind: PhaseKind) -> Value {
    json!({ "run_id": run_id, "phase": kind.name() })
}

/// The payload of a run-started event.
#[must_use]
pub fn started(run_id: &str, target: &str) -> Value {
    json!({ "run_id": run_id, "target": target })
}

/// The payload of a phase-transition event.
#[must_use]
pub fn phase(run_id: &str, kind: PhaseKind, status: &str) -> Value {
    json!({ "run_id": run_id, "phase": kind.name(), "status": status })
}

/// The payload of a finding event.
#[must_use]
pub fn finding(run_id: &str, finding: &Value) -> Value {
    json!({ "run_id": run_id, "finding": finding })
}

/// The payload of a refuted-hypothesis event.
#[must_use]
pub fn refuted(run_id: &str, hypothesis: &str) -> Value {
    json!({ "run_id": run_id, "hypothesis": hypothesis })
}

/// The payload of a checkpoint event.
#[must_use]
pub fn checkpoint(run_id: &str, kind: PhaseKind) -> Value {
    json!({ "run_id": run_id, "phase": kind.name() })
}

/// The session-entry body opening one run.
#[must_use]
pub fn started_entry(run_id: &str, target: &str) -> Value {
    json!({ "run_id": run_id, "target": target, "status": STATUS_STARTED })
}

/// The session-entry body of one phase transition.
#[must_use]
pub fn phase_entry(run_id: &str, kind: PhaseKind, status: &str, findings: usize) -> Value {
    json!({
        "run_id": run_id,
        "phase": kind.name(),
        "status": status,
        "findings": findings,
    })
}

#[cfg(test)]
mod tests {
    //! Unit tests for the event contract.

    use super::{
        CHECKPOINT, COMPLETED, FAILED, FINDING, PHASE, REFUTED, RUNNING, STARTED, checkpoint,
        finding, phase, phase_entry, refuted, started, started_entry,
    };
    use crate::pipeline::PhaseKind;
    use serde_json::json;

    /// Every channel is the literal contract name.
    #[test]
    fn the_channels_are_the_contract() {
        assert_eq!(STARTED, "maestro.security.run.started");
        assert_eq!(PHASE, "maestro.security.run.phase");
        assert_eq!(FINDING, "maestro.security.run.finding");
        assert_eq!(REFUTED, "maestro.security.run.refuted");
        assert_eq!(CHECKPOINT, "maestro.security.run.checkpoint");
        assert_eq!(RUNNING, "running");
        assert_eq!(COMPLETED, "completed");
        assert_eq!(FAILED, "failed");
    }

    /// The started payload names the run and its target.
    #[test]
    fn the_started_payload_names_the_run() {
        let payload = started("run-1-x", "example.com");
        assert_eq!(payload["run_id"], "run-1-x");
        assert_eq!(payload["target"], "example.com");
    }

    /// The phase payload carries the report phase name and the status.
    #[test]
    fn the_phase_payload_carries_the_phase_name() {
        let payload = phase("run-1-x", PhaseKind::Model, COMPLETED);
        assert_eq!(payload["run_id"], "run-1-x");
        assert_eq!(payload["phase"], "target-model");
        assert_eq!(payload["status"], "completed");
    }

    /// The finding payload embeds the finding document unchanged.
    #[test]
    fn the_finding_payload_embeds_the_finding() {
        let document = json!({ "id": "F-001", "class": "sqli" });
        let payload = finding("run-1-x", &document);
        assert_eq!(payload["run_id"], "run-1-x");
        assert_eq!(payload["finding"]["id"], "F-001");
        assert_eq!(payload["finding"]["class"], "sqli");
    }

    /// The refuted payload carries the hypothesis text.
    #[test]
    fn the_refuted_payload_carries_the_hypothesis() {
        let payload = refuted("run-1-x", "no injection on /login");
        assert_eq!(payload["run_id"], "run-1-x");
        assert_eq!(payload["hypothesis"], "no injection on /login");
    }

    /// The checkpoint payload names the phase that was committed.
    #[test]
    fn the_checkpoint_payload_names_the_phase() {
        let payload = checkpoint("run-1-x", PhaseKind::Verify);
        assert_eq!(payload["run_id"], "run-1-x");
        assert_eq!(payload["phase"], "verify");
    }

    /// The started entry opens the subagent-like run thread.
    #[test]
    fn the_started_entry_opens_the_thread() {
        let body = started_entry("run-1-x", "example.com");
        assert_eq!(body["run_id"], "run-1-x");
        assert_eq!(body["target"], "example.com");
        assert_eq!(body["status"], "started");
    }

    /// A phase entry carries the phase, its status and the running count.
    #[test]
    fn a_phase_entry_carries_the_phase_and_count() {
        let body = phase_entry("run-1-x", PhaseKind::Verify, COMPLETED, 2);
        assert_eq!(body["run_id"], "run-1-x");
        assert_eq!(body["phase"], "verify");
        assert_eq!(body["status"], "completed");
        assert_eq!(body["findings"], 2);
    }
}
