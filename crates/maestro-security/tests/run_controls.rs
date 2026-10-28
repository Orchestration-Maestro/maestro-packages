//! Stop and steering contracts through the public lifecycle.

use std::future::Future;
use std::task::{Context, Poll, Waker};

use maestro_extensions_wasm::ExtensionFuture;
use maestro_security::autonomy::run::PhaseKind;
use maestro_security::autonomy::run::{
    Control, EventSink, Plan, SessionEntry, StepReport, drive_controlled,
};
use serde_json::Value;

/// Poll a lifecycle whose ports complete without waiting.
fn ready<T>(future: impl Future<Output = T>) -> T {
    let mut future = Box::pin(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
        std::thread::yield_now();
    }
}

/// Collect events and optionally stop at the first checkpoint.
#[derive(Default)]
struct Bus {
    /// Delivered events.
    events: Vec<(String, Value)>,
    /// Control to stop at a phase boundary.
    stop_at_checkpoint: Option<Control>,
}

impl EventSink for Bus {
    fn emit<'a>(&'a mut self, channel: &'a str, payload: Value) -> ExtensionFuture<'a, ()> {
        if channel == "maestro.security.run.checkpoint"
            && let Some(control) = &self.stop_at_checkpoint
        {
            control.stop();
        }
        self.events.push((channel.into(), payload));
        Box::pin(async { Ok(()) })
    }
}

/// Collect curated entries.
#[derive(Default)]
struct Entries(Vec<Value>);

impl SessionEntry for Entries {
    fn append(&mut self, body: &Value) -> Result<(), String> {
        self.0.push(body.clone());
        Ok(())
    }
}

/// The public plan for a two-phase assessment.
fn plan() -> Plan<'static> {
    Plan {
        run_id: "run-controls",
        target: "example.com",
        phases: &[PhaseKind::Recon, PhaseKind::Model],
    }
}

/// Guidance sent while recon runs reaches the next running phase.
#[test]
fn steering_reaches_the_running_phase_and_emits_steered() {
    let control = Control::default();
    let sender = control.clone();
    let mut observed = Vec::new();
    let mut step = |phase, notes: &[String]| {
        if phase == PhaseKind::Recon {
            sender.steer("user", "check the API");
        }
        observed.push((phase, notes.to_vec()));
        Ok(StepReport::default())
    };
    let mut bus = Bus::default();
    ready(drive_controlled(
        &plan(),
        &mut bus,
        &mut Entries::default(),
        &control,
        &mut step,
    ))
    .expect("drive");
    assert!(observed[0].1.is_empty());
    assert_eq!(observed[1].1, ["check the API"]);
    let steered: Vec<_> = bus
        .events
        .iter()
        .filter(|(channel, _)| channel == "maestro.security.run.steered")
        .collect();
    assert_eq!(steered.len(), 1);
    assert_eq!(steered[0].1["run_id"], "run-controls");
    assert_eq!(steered[0].1["source"], "user");
    assert_eq!(steered[0].1["message"], "check the API");
}

/// A stop issued during a phase ends the run before the next phase.
#[test]
fn stop_during_phase_emits_killed_without_result() {
    let control = Control::default();
    let sender = control.clone();
    let mut phases = Vec::new();
    let mut step = |phase, _: &[String]| {
        phases.push(phase);
        sender.stop();
        Ok(StepReport::default())
    };
    let mut bus = Bus::default();
    let mut entries = Entries::default();
    let outcome = ready(drive_controlled(
        &plan(),
        &mut bus,
        &mut entries,
        &control,
        &mut step,
    ));
    assert_eq!(
        outcome.expect("drive"),
        maestro_security::autonomy::run::Outcome::Killed
    );
    assert_eq!(phases, [PhaseKind::Recon]);
    assert!(bus.events.iter().any(
        |(channel, payload)| channel == "maestro.security.run.killed"
            && payload["run_id"] == "run-controls"
    ));
    assert!(
        !bus.events
            .iter()
            .any(|(channel, _)| channel == "maestro.security.assessment")
    );
    assert_eq!(entries.0.last().expect("killed entry")["status"], "killed");
}

/// A stop arriving through checkpoint delivery prevents the next phase.
#[test]
fn stop_is_serviced_between_phases() {
    let control = Control::default();
    let mut bus = Bus {
        stop_at_checkpoint: Some(control.clone()),
        ..Bus::default()
    };
    let mut phases = Vec::new();
    let mut step = |phase, _: &[String]| {
        phases.push(phase);
        Ok(StepReport::default())
    };
    let outcome = ready(drive_controlled(
        &plan(),
        &mut bus,
        &mut Entries::default(),
        &control,
        &mut step,
    ));
    assert_eq!(
        outcome.expect("drive"),
        maestro_security::autonomy::run::Outcome::Killed
    );
    assert_eq!(phases, [PhaseKind::Recon]);
    assert_eq!(
        bus.events.last().expect("killed event").0,
        "maestro.security.run.killed"
    );
}

/// Process-isolated fixture shared with capability tests.
#[path = "support/handler.rs"]
mod handler;

/// Return runner evidence and assert that guidance actually reaches the model.
fn guided_phase(path: &str, body: &Value) -> (u16, String) {
    match path {
        "/health" => (200, "{}".into()),
        "/models" => handler::models(),
        "/exec" => handler::execution("retained recon evidence", 0),
        "/chat/completions" => {
            let system = body["messages"][0]["content"].as_str().unwrap_or_default();
            assert!(system.contains("Untrusted steering guidance"));
            assert!(system.contains("check the API"));
            handler::completion("{\"services\":[]}")
        }
        _ => (404, format!("unexpected request {path}")),
    }
}

/// Send guidance after recon and stop after the model phase commits.
struct GuidedBus {
    /// Shared run control.
    control: Control,
    /// Delivered events.
    events: Vec<(String, Value)>,
    /// Number of committed phases.
    checkpoints: usize,
}

impl EventSink for GuidedBus {
    fn emit<'a>(&'a mut self, channel: &'a str, payload: Value) -> ExtensionFuture<'a, ()> {
        if channel == "maestro.security.run.checkpoint" {
            self.checkpoints += 1;
            if self.checkpoints == 1 {
                self.control.steer("agent", "check the API");
            } else {
                self.control.stop();
            }
        }
        self.events.push((channel.into(), payload));
        Box::pin(async { Ok(()) })
    }
}

/// The real run surface consumes guidance, keeps evidence and settles as killed.
#[test]
fn triggered_run_keeps_evidence_and_has_no_result_after_stop()
-> Result<(), Box<dyn std::error::Error>> {
    if !handler::isolated(
        "triggered_run_keeps_evidence_and_has_no_result_after_stop",
        guided_phase,
    )? {
        return Ok(());
    }
    let id = maestro_security::autonomy::start_run("example.test")?;
    let mut bus = GuidedBus {
        control: maestro_security::autonomy::steer::for_run(&id)?,
        events: Vec::new(),
        checkpoints: 0,
    };
    ready(maestro_security::autonomy::run_trigger(
        &mut bus,
        &mut Entries::default(),
        &maestro_security::autonomy::trigger_payload(&id, "example.test"),
    ))?;
    assert_eq!(bus.checkpoints, 2);
    assert!(bus.events.iter().any(
        |(channel, payload)| channel == "maestro.security.run.steered"
            && payload["source"] == "agent"
    ));
    assert_eq!(
        bus.events.last().expect("killed").0,
        "maestro.security.run.killed"
    );
    assert!(
        !bus.events
            .iter()
            .any(|(channel, _)| channel == "maestro.security.assessment")
    );
    assert!(maestro_security::load_report(&id).is_err());
    assert!(maestro_security::evidence_for(&id)?.contains("retained recon evidence"));
    let record = maestro_security::Store.get_run(&id)?.expect("run record");
    assert_eq!(record.status, maestro_security::Status::Killed);
    assert!(record.finished_at.is_some());
    assert!(maestro_security::autonomy::steer::for_run(&id).is_err());
    Ok(())
}

/// Only the five most recent queued notes reach a phase, in order.
#[test]
fn steering_retains_the_five_most_recent_notes() {
    let control = Control::default();
    for index in 0..7 {
        control.steer("user", &format!("note-{index}"));
    }
    let mut observed = Vec::new();
    let mut step = |_, notes: &[String]| {
        observed.push(notes.to_vec());
        Ok(StepReport::default())
    };
    let mut bus = Bus::default();
    ready(drive_controlled(
        &plan(),
        &mut bus,
        &mut Entries::default(),
        &control,
        &mut step,
    ))
    .expect("drive");
    assert_eq!(
        observed,
        vec![vec!["note-2", "note-3", "note-4", "note-5", "note-6"]; 2]
    );
    assert_eq!(
        bus.events
            .iter()
            .filter(|(channel, _)| channel == "maestro.security.run.steered")
            .count(),
        5
    );
}

/// A stop in the final phase must not be mistaken for successful completion.
#[test]
fn stop_in_final_phase_is_killed() {
    let control = Control::default();
    let mut step = |phase, _: &[String]| {
        if phase == PhaseKind::Model {
            control.stop();
        }
        Ok(StepReport::default())
    };
    let outcome = ready(drive_controlled(
        &plan(),
        &mut Bus::default(),
        &mut Entries::default(),
        &control,
        &mut step,
    ))
    .expect("drive");
    assert_eq!(outcome, maestro_security::autonomy::run::Outcome::Killed);
}

/// An entry sink that refuses only the terminal killed notification.
struct RejectKilledEntry;

impl SessionEntry for RejectKilledEntry {
    fn append(&mut self, body: &Value) -> Result<(), String> {
        if body["status"] == "killed" {
            Err("killed entry refused".into())
        } else {
            Ok(())
        }
    }
}

/// Rejected terminal entries must not discard already collected evidence.
#[test]
fn killed_entry_failure_keeps_evidence() -> Result<(), Box<dyn std::error::Error>> {
    if !handler::isolated("killed_entry_failure_keeps_evidence", guided_phase)? {
        return Ok(());
    }
    let id = maestro_security::autonomy::start_run("entry-failure.test")?;
    let mut bus = GuidedBus {
        control: maestro_security::autonomy::steer::for_run(&id)?,
        events: Vec::new(),
        checkpoints: 0,
    };
    let error = ready(maestro_security::autonomy::run::execute_controlled(
        &mut bus,
        &mut RejectKilledEntry,
        &maestro_security::AssessParams {
            target: "entry-failure.test".into(),
            ..Default::default()
        },
        &id,
        &maestro_security::autonomy::steer::for_run(&id)?,
    ))
    .unwrap_err();
    assert_eq!(error, "killed entry refused");
    assert!(maestro_security::evidence_for(&id)?.contains("retained recon evidence"));
    assert!(maestro_security::load_report(&id).is_err());
    maestro_security::autonomy::finish_run(&id, Err(&error))?;
    Ok(())
}

/// A bridge response whose report output can be distinguished in the ledger.
fn report_phase(path: &str, _: &Value) -> (u16, String) {
    match path {
        "/health" => (200, "{}".into()),
        "/models" => handler::models(),
        "/exec" => handler::execution("retained recon evidence", 0),
        "/chat/completions" => handler::completion("{\"summary\":\"report-phase evidence\"}"),
        _ => (404, format!("unexpected request {path}")),
    }
}

/// Stop after the real report call, before its completed report is published.
#[test]
fn report_phase_stop_does_not_retain_a_completed_report() -> Result<(), Box<dyn std::error::Error>>
{
    if !handler::isolated(
        "report_phase_stop_does_not_retain_a_completed_report",
        report_phase,
    )? {
        return Ok(());
    }
    let id = maestro_security::autonomy::start_run("report-stop.test")?;
    let control = maestro_security::autonomy::steer::for_run(&id)?;
    let params = maestro_security::AssessParams {
        target: "report-stop.test".into(),
        agent: Some("maestro.rebuild".into()),
        ..Default::default()
    };
    let mut bus = Bus::default();
    let mut entries = Entries::default();
    let mut lifecycle = Box::pin(maestro_security::autonomy::run::execute_controlled(
        &mut bus,
        &mut entries,
        &params,
        &id,
        &control,
    ));
    let result = ready(std::future::poll_fn(|cx| {
        let result = lifecycle.as_mut().poll(cx);
        if result.is_pending()
            && maestro_security::evidence_for(&id)
                .is_ok_and(|ledger| ledger.contains("\"phase\": \"report\""))
        {
            control.stop();
        }
        result
    }))?;
    drop(lifecycle);
    assert!(result.is_none(), "report-phase stop must kill the run");
    assert!(maestro_security::load_report(&id).is_err());
    assert!(maestro_security::evidence_for(&id)?.contains("\"phase\": \"report\""));
    assert_eq!(entries.0.last().expect("killed entry")["status"], "killed");
    assert_eq!(
        bus.events.last().expect("killed event").0,
        "maestro.security.run.killed"
    );
    maestro_security::autonomy::finish_run(&id, Err("test ended"))?;
    Ok(())
}
