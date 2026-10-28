//! Stop and steering contracts through the public lifecycle.

use std::future::Future;
use std::task::{Context, Poll, Waker};

use maestro_extensions_wasm::ExtensionFuture;
use maestro_security::autonomy::run::{
    Control, EventSink, Plan, SessionEntry, StepReport, drive_controlled,
};
use maestro_security::pipeline::PhaseKind;
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
