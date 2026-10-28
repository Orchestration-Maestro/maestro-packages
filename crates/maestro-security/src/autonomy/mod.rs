//! The autonomous assessment trigger and run registry.
//!
//! Only an explicit command starts a run. `/assess <target>` records the run
//! in the run index and the host wiring emits the trigger event; the trigger
//! listener's tail runs the lifecycle in [`run`] under that same run id.
//! Run controls are registered only when the run machinery implements them.

use serde_json::{Value, json};

pub mod events;
pub mod run;
pub mod steer;

use crate::pipeline;
use crate::report::Assessment;
use crate::store::{RunRecord, Store};

/// The command that starts an assessment.
pub const ASSESS_COMMAND: &str = "assess";
/// The command that stops a running assessment.
pub const STOP_COMMAND: &str = "security-stop";
/// The command that steers a running assessment.
pub const STEER_COMMAND: &str = "security-steer";
/// The bus channel carrying one trigger from a command to its listener.
///
/// Internal machinery: not one of the eight public contract channels in
/// [`events`]; only the command and its listener share it.
pub const TRIGGER_CHANNEL: &str = "maestro.security.trigger";

/// One registered chat command.
#[derive(Clone, Copy)]
pub struct Command {
    /// The command name, without its leading slash.
    pub name: &'static str,
    /// The one-line description shown to users.
    pub description: &'static str,
}

/// The trigger commands, in registration order.
pub const COMMANDS: [Command; 3] = [
    Command {
        name: ASSESS_COMMAND,
        description: "Assess a target in the background; answers with the run id",
    },
    Command {
        name: STOP_COMMAND,
        description: "Stop a running assessment at the next phase boundary",
    },
    Command {
        name: STEER_COMMAND,
        description: "Send guidance to a running assessment",
    },
];

/// The target of `/assess`, trimmed.
///
/// # Errors
/// Fails when the argument is blank.
pub fn parse_target(args: &str) -> Result<String, String> {
    let target = args.trim();
    if target.is_empty() {
        return Err(usage(ASSESS_COMMAND, "<target>"));
    }
    Ok(target.to_string())
}

/// The run id of `/security-stop`, trimmed.
///
/// # Errors
/// Fails when the argument is blank.
pub fn parse_run_id(args: &str) -> Result<String, String> {
    let run_id = args.trim();
    if run_id.is_empty() {
        return Err(usage(STOP_COMMAND, "<run_id>"));
    }
    Ok(run_id.to_string())
}

/// The run id and steering message of `/security-steer`.
///
/// # Errors
/// Fails when the run id or the message is missing.
pub fn parse_steer(args: &str) -> Result<(String, String), String> {
    let Some((run_id, message)) = args.trim().split_once(char::is_whitespace) else {
        return Err(usage(STEER_COMMAND, "<run_id> <message>"));
    };
    let message = message.trim();
    if run_id.is_empty() || message.is_empty() {
        return Err(usage(STEER_COMMAND, "<run_id> <message>"));
    }
    Ok((run_id.to_string(), message.to_string()))
}

/// The usage line of one command.
fn usage(command: &str, arguments: &str) -> String {
    format!("usage: /{command} {arguments}")
}

/// The trigger payload for one run.
#[must_use]
pub fn trigger_payload(run_id: &str, target: &str) -> Value {
    json!({ "run_id": run_id, "target": target })
}

/// The run-index record for a run that is about to start.
#[must_use]
pub fn new_run_record(target: &str, model: &str) -> RunRecord {
    RunRecord::starting(
        pipeline::new_run_id(target),
        target,
        crate::agents::resolve_agent(None).name,
        model,
        None,
    )
}

/// Record a starting run in the run index and return its id.
///
/// # Errors
/// Fails when the run index cannot be written.
pub fn start_run(target: &str) -> Result<String, String> {
    let record = new_run_record(target.trim(), &pipeline::Config::from_env().default_model);
    Store.upsert_run(&record)?;
    steer::register(&record.run_id);
    Ok(record.run_id)
}

/// Record a run's final state in the run index.
///
/// # Errors
/// Fails when the run is unknown or the index cannot be written.
pub fn finish_run(run_id: &str, outcome: Result<&Assessment, &str>) -> Result<(), String> {
    let mut record = Store
        .get_run(run_id)?
        .ok_or_else(|| format!("no run {run_id} in the index"))?;
    record.settle(outcome);
    Store.upsert_run(&record)?;
    steer::remove(run_id);
    Ok(())
}

/// Run the lifecycle for one trigger payload and settle its record.
///
/// The tail returns its failure to the host reporter, so a listener keeps
/// working after one bad run.
///
/// # Errors
/// Fails when the payload lacks a run id or target, or the record cannot be
/// settled.
pub async fn run_trigger<S: run::EventSink, E: run::SessionEntry>(
    sink: &mut S,
    entry: &mut E,
    payload: &Value,
) -> Result<(), String> {
    let (run_id, target) =
        decode_trigger(payload).ok_or("trigger payload needs run_id and target")?;
    let params = pipeline::AssessParams {
        target,
        agent: None,
        model: None,
        max_hypotheses: None,
        pattern: None,
    };
    let control = steer::for_run(&run_id)?;
    match run::execute_controlled(sink, entry, &params, &run_id, &control).await {
        Ok(Some(report)) => finish_run(&run_id, Ok(&report)),
        Ok(None) => kill_run(&run_id),
        Err(error) => finish_run(&run_id, Err(&error)),
    }
}

/// Settle a killed run without a report or a result event.
fn kill_run(run_id: &str) -> Result<(), String> {
    let mut record = Store
        .get_run(run_id)?
        .ok_or_else(|| format!("no run {run_id} in the index"))?;
    record.settle(Err("killed"));
    record.status = crate::store::Status::Killed;
    Store.upsert_run(&record)?;
    steer::remove(run_id);
    Ok(())
}

/// The run id and target of one trigger payload.
fn decode_trigger(payload: &Value) -> Option<(String, String)> {
    let run_id = payload.get("run_id")?.as_str()?;
    let target = payload.get("target")?.as_str()?;
    Some((run_id.to_string(), target.to_string()))
}

#[cfg(test)]
mod tests {
    //! Unit tests for the trigger surface.

    use super::{
        COMMANDS, finish_run, new_run_record, parse_run_id, parse_steer, parse_target, start_run,
        trigger_payload,
    };
    use crate::store::{Status, Store};
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Only functional commands are offered, each described.
    #[test]
    fn the_command_table_contains_only_functional_commands() {
        let names: Vec<&str> = COMMANDS.iter().map(|command| command.name).collect();
        assert_eq!(names, ["assess", "security-stop", "security-steer"]);
        assert!(
            COMMANDS
                .iter()
                .all(|command| !command.description.trim().is_empty()),
            "every command needs a description"
        );
    }

    /// A blank target is a usage error naming the command.
    #[test]
    fn assess_requires_a_target() {
        let error = parse_target("   ").unwrap_err();
        assert!(
            error.contains("usage: /assess <target>"),
            "unexpected: {error}"
        );
    }

    /// A target comes back trimmed.
    #[test]
    fn assess_takes_the_target() {
        assert_eq!(
            parse_target("  https://example.com ").unwrap(),
            "https://example.com"
        );
    }

    /// Stop needs the run id and trims it.
    #[test]
    fn stop_requires_a_run_id() {
        let error = parse_run_id("").unwrap_err();
        assert!(
            error.contains("usage: /security-stop <run_id>"),
            "unexpected: {error}"
        );
        assert_eq!(parse_run_id(" run-1-x ").unwrap(), "run-1-x");
    }

    /// Steer needs a run id and a message.
    #[test]
    fn steer_requires_a_run_id_and_message() {
        assert!(parse_steer("run-1").is_err(), "message is required");
        assert!(parse_steer("run-1   ").is_err(), "message is required");
        assert!(parse_steer("   ").is_err(), "run id is required");
        let (run_id, message) = parse_steer("run-1 check the api").unwrap();
        assert_eq!(run_id, "run-1");
        assert_eq!(message, "check the api");
    }

    /// The emitted payload names the run and its target.
    #[test]
    fn trigger_payload_names_the_run() {
        let payload = trigger_payload("run-1-x", "example.com");
        assert_eq!(payload["run_id"], "run-1-x");
        assert_eq!(payload["target"], "example.com");
    }

    /// A starting record is running, unfinished and empty.
    #[test]
    fn a_starting_record_is_running() {
        let record = new_run_record("example.com", "qwythos");
        assert!(record.run_id.starts_with("run-"), "id: {}", record.run_id);
        assert_eq!(record.target, "example.com");
        assert_eq!(record.model, "qwythos");
        assert!(matches!(record.status, Status::Running));
        assert!(record.finished_at.is_none());
        assert_eq!(record.findings, 0);
        assert_eq!(record.refuted, 0);
    }

    /// A failed outcome settles the record as failed with its cause.
    #[test]
    fn a_failed_outcome_settles_the_record() {
        let mut record = new_run_record("example.com", "qwythos");
        record.settle(Err("runner down"));
        assert!(matches!(record.status, Status::Failed));
        assert_eq!(record.summary, "runner down");
        assert!(record.finished_at.is_some());
    }

    /// A started run lands in the run index under its returned id.
    #[test]
    fn a_started_run_is_recorded() {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let home = std::env::temp_dir().join(format!(
            "maestro-security-index-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        crate::evidence::with_home(&home, || {
            let run_id = start_run("registry-test.example").expect("start_run");
            let runs = Store.list_runs(0).expect("run index");
            assert!(
                runs.iter().any(|run| run.run_id == run_id),
                "run {run_id} missing from the index"
            );
            finish_run(&run_id, Err("test cleanup")).expect("settle the test run");
        });
        let _ = std::fs::remove_dir_all(&home);
    }

    /// A payload without a run id or target decodes to nothing.
    #[test]
    fn a_short_payload_is_rejected() {
        assert!(
            super::decode_trigger(&serde_json::json!({ "run_id": "run-1" })).is_none(),
            "a payload needs both the run id and the target"
        );
    }
}
