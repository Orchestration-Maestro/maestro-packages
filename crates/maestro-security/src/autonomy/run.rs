//! The run lifecycle: step the phases, emit the event contract and keep the
//! run's session entry current.

use maestro_extensions_wasm::ExtensionFuture;
use serde_json::Value;

use crate::pipeline::{self, AssessParams, PhaseKind};
use crate::report::Assessment;

use super::events;

/// One run's identity and phase plan.
pub struct Plan<'a> {
    /// The run id the record and every event carry.
    pub run_id: &'a str,
    /// The assessed target.
    pub target: &'a str,
    /// The phases this run executes, in order.
    pub phases: &'a [PhaseKind],
}

/// What one phase produced for the lifecycle.
#[derive(Default)]
pub struct StepReport {
    /// Findings this phase added, serialized for the bus.
    pub findings: Vec<Value>,
    /// Hypotheses this phase refuted.
    pub refuted: Vec<String>,
}

/// Receives the lifecycle's bus events.
pub trait EventSink {
    /// Emit one contract event.
    fn emit<'a>(&'a mut self, channel: &'a str, payload: Value) -> ExtensionFuture<'a, ()>;
}

/// Receives the run's live session entries.
pub trait SessionEntry {
    /// Append one run entry.
    ///
    /// # Errors
    /// Returns the host rejection.
    fn append(&mut self, body: &Value) -> Result<(), String>;
}

/// Drive one run: the started event, then each phase with its findings,
/// refutations and checkpoint, stopping at the first failing phase.
///
/// # Errors
/// Returns the failing step's error after its failed status is emitted.
pub async fn drive<S: EventSink, E: SessionEntry>(
    plan: &Plan<'_>,
    sink: &mut S,
    entry: &mut E,
    step: &mut dyn FnMut(PhaseKind) -> Result<StepReport, String>,
) -> Result<(), String> {
    let run_id = plan.run_id;
    sink.emit(events::STARTED, events::started(run_id, plan.target))
        .await?;
    entry.append(&events::started_entry(run_id, plan.target))?;
    let mut findings = 0usize;
    for kind in plan.phases {
        let kind = *kind;
        entry.append(&events::phase_entry(
            run_id,
            kind,
            events::RUNNING,
            findings,
        ))?;
        match step(kind) {
            Ok(report) => {
                findings += report.findings.len();
                sink.emit(
                    events::PHASE,
                    events::phase(run_id, kind, events::COMPLETED),
                )
                .await?;
                for finding in &report.findings {
                    sink.emit(events::FINDING, events::finding(run_id, finding))
                        .await?;
                }
                for hypothesis in &report.refuted {
                    sink.emit(events::REFUTED, events::refuted(run_id, hypothesis))
                        .await?;
                }
                sink.emit(events::CHECKPOINT, events::checkpoint(run_id, kind))
                    .await?;
                entry.append(&events::phase_entry(
                    run_id,
                    kind,
                    events::COMPLETED,
                    findings,
                ))?;
            }
            Err(error) => {
                sink.emit(events::PHASE, events::phase(run_id, kind, events::FAILED))
                    .await?;
                entry.append(&events::phase_entry(run_id, kind, events::FAILED, findings))?;
                return Err(error);
            }
        }
    }
    Ok(())
}

/// Run one assessment as a live lifecycle: the engine steps the phases and
/// this driver emits the contract and keeps the run's session entry current.
///
/// # Errors
/// Fails if the target is empty, the runner preflight fails, a phase fails,
/// or the report cannot be retained.
pub async fn execute<S: EventSink, E: SessionEntry>(
    sink: &mut S,
    entry: &mut E,
    params: &AssessParams,
    run_id: &str,
) -> Result<Assessment, String> {
    let (engine, agent, opts) = pipeline::assessment_setup(params)?;
    let (ctx, mut state) = engine.prepare(agent, &params.target, &opts)?;
    let phases: Vec<PhaseKind> = pipeline::planned_phases(agent).collect();
    let plan = Plan {
        run_id,
        target: &params.target,
        phases: &phases,
    };
    let mut seen_findings = 0usize;
    let mut seen_refuted = 0usize;
    let mut step = |kind: PhaseKind| -> Result<StepReport, String> {
        engine.step(kind, &ctx, &mut state)?;
        let findings = state.findings[seen_findings..]
            .iter()
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        let refuted = state.refuted[seen_refuted..]
            .iter()
            .map(|refuted| refuted.hypothesis.clone())
            .collect();
        seen_findings = state.findings.len();
        seen_refuted = state.refuted.len();
        Ok(StepReport { findings, refuted })
    };
    drive(&plan, sink, entry, &mut step).await?;
    engine.finish(run_id, agent, &ctx, &mut state)
}

#[cfg(test)]
mod tests {
    //! Unit tests for the lifecycle driver.

    use std::cell::RefCell;
    use std::future::Future;
    use std::rc::Rc;
    use std::task::{Context, Poll, Waker};

    use maestro_extensions_wasm::ExtensionFuture;
    use serde_json::{Value, json};

    use super::{EventSink, Plan, SessionEntry, StepReport, drive};
    use crate::autonomy::events;
    use crate::pipeline::PhaseKind;

    /// Poll one future that never waits on real input.
    fn ready<T>(future: impl Future<Output = T>) -> T {
        let mut future = Box::pin(future);
        let mut cx = Context::from_waker(Waker::noop());
        loop {
            match future.as_mut().poll(&mut cx) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    /// Records the bus events in emission order.
    #[derive(Default, Clone)]
    struct BusRecorder(Rc<RefCell<Vec<(String, Value)>>>);

    impl BusRecorder {
        /// The channels emitted so far.
        fn channels(&self) -> Vec<String> {
            self.0.borrow().iter().map(|(c, _)| c.clone()).collect()
        }
        /// The payload of the last event on one channel.
        fn last(&self, channel: &str) -> Value {
            self.0
                .borrow()
                .iter()
                .rev()
                .find(|(c, _)| c == channel)
                .map_or(Value::Null, |(_, payload)| payload.clone())
        }
    }

    impl EventSink for BusRecorder {
        fn emit<'a>(&'a mut self, channel: &'a str, payload: Value) -> ExtensionFuture<'a, ()> {
            let events = Rc::clone(&self.0);
            let channel = channel.to_string();
            Box::pin(async move {
                events.borrow_mut().push((channel, payload));
                Ok(())
            })
        }
    }

    /// Records the session entries in append order.
    #[derive(Default, Clone)]
    struct EntryRecorder(Rc<RefCell<Vec<Value>>>);

    impl EntryRecorder {
        /// The entry bodies appended so far.
        fn bodies(&self) -> Vec<Value> {
            self.0.borrow().clone()
        }
    }

    impl SessionEntry for EntryRecorder {
        fn append(&mut self, body: &Value) -> Result<(), String> {
            self.0.borrow_mut().push(body.clone());
            Ok(())
        }
    }

    /// The scripted step used by the lifecycle tests.
    fn scripted(
        outcomes: Vec<Result<StepReport, String>>,
    ) -> impl FnMut(PhaseKind) -> Result<StepReport, String> {
        let mut queue = outcomes.into_iter();
        move |_phase| queue.next().unwrap_or_else(|| Ok(StepReport::default()))
    }

    /// The plan used by the lifecycle tests.
    fn plan(phases: &[PhaseKind]) -> Plan<'_> {
        Plan {
            run_id: "run-1-x",
            target: "example.com",
            phases,
        }
    }

    /// A run opens with the started event, then each phase commits in order.
    #[test]
    fn the_lifecycle_opens_then_commits_each_phase() {
        let mut bus = BusRecorder::default();
        let mut entry = EntryRecorder::default();
        let mut step = scripted(Vec::new());
        ready(drive(
            &plan(&[PhaseKind::Recon, PhaseKind::Model]),
            &mut bus,
            &mut entry,
            &mut step,
        ))
        .expect("drive");
        assert_eq!(
            bus.channels(),
            vec![
                events::STARTED,
                events::PHASE,
                events::CHECKPOINT,
                events::PHASE,
                events::CHECKPOINT
            ],
            "each phase emits exactly one transition and one checkpoint"
        );
        assert_eq!(bus.last(events::STARTED)["run_id"], "run-1-x");
        assert_eq!(bus.last(events::STARTED)["target"], "example.com");
        assert_eq!(bus.last(events::CHECKPOINT)["phase"], "target-model");
    }

    /// Findings and refutations of a phase emit their own events in order.
    #[test]
    fn findings_and_refutations_emit_their_events() {
        let mut bus = BusRecorder::default();
        let mut entry = EntryRecorder::default();
        let report = StepReport {
            findings: vec![json!({ "id": "F-001", "class": "sqli" })],
            refuted: vec!["no injection on /login".into()],
        };
        let mut step = scripted(vec![Ok(report)]);
        ready(drive(
            &plan(&[PhaseKind::Verify]),
            &mut bus,
            &mut entry,
            &mut step,
        ))
        .expect("drive");
        assert_eq!(
            bus.channels(),
            vec![
                events::STARTED,
                events::PHASE,
                events::FINDING,
                events::REFUTED,
                events::CHECKPOINT
            ]
        );
        assert_eq!(bus.last(events::FINDING)["finding"]["class"], "sqli");
        assert_eq!(
            bus.last(events::REFUTED)["hypothesis"],
            "no injection on /login"
        );
    }

    /// A phase with nothing to report emits no finding or refuted event.
    #[test]
    fn no_event_fires_without_its_transition() {
        let mut bus = BusRecorder::default();
        let mut entry = EntryRecorder::default();
        let mut step = scripted(Vec::new());
        ready(drive(
            &plan(&[PhaseKind::Recon]),
            &mut bus,
            &mut entry,
            &mut step,
        ))
        .expect("drive");
        assert!(
            !bus.channels().contains(&events::FINDING.to_string()),
            "no finding event without a finding"
        );
        assert!(
            !bus.channels().contains(&events::REFUTED.to_string()),
            "no refuted event without a refutation"
        );
    }

    /// A failing phase emits the failed status and nothing after it.
    #[test]
    fn a_failing_phase_reports_failed_and_stops() {
        let mut bus = BusRecorder::default();
        let mut entry = EntryRecorder::default();
        let mut step = scripted(vec![Ok(StepReport::default()), Err("runner down".into())]);
        let result = ready(drive(
            &plan(&[PhaseKind::Recon, PhaseKind::Model, PhaseKind::Hypotheses]),
            &mut bus,
            &mut entry,
            &mut step,
        ));
        assert_eq!(result.unwrap_err(), "runner down");
        assert_eq!(
            bus.channels(),
            vec![
                events::STARTED,
                events::PHASE,
                events::CHECKPOINT,
                events::PHASE
            ],
            "the failing phase emits its phase event and stops the run"
        );
        assert_eq!(bus.last(events::PHASE)["status"], "failed");
        assert_eq!(bus.last(events::PHASE)["phase"], "target-model");
    }

    /// The run entry opens with the run id and tracks the finding count.
    #[test]
    fn the_entry_opens_and_tracks_findings() {
        let mut bus = BusRecorder::default();
        let mut entry = EntryRecorder::default();
        let report = StepReport {
            findings: vec![json!({ "id": "F-001" }), json!({ "id": "F-002" })],
            refuted: Vec::new(),
        };
        let mut step = scripted(vec![Ok(report)]);
        ready(drive(
            &plan(&[PhaseKind::Verify]),
            &mut bus,
            &mut entry,
            &mut step,
        ))
        .expect("drive");
        let bodies = entry.bodies();
        assert_eq!(bodies.first().expect("started entry")["run_id"], "run-1-x");
        assert_eq!(bodies.first().expect("started entry")["status"], "started");
        let completed = bodies.last().expect("completed entry");
        assert_eq!(completed["status"], "completed");
        assert_eq!(completed["phase"], "verify");
        assert_eq!(completed["findings"], 2);
    }
    /// A bus that rejects one selected event.
    struct RejectingBus {
        /// Event whose delivery the host refuses.
        reject: &'static str,
        /// Events attempted before the driver returns.
        attempted: Vec<String>,
    }

    impl EventSink for RejectingBus {
        fn emit<'a>(&'a mut self, channel: &'a str, _payload: Value) -> ExtensionFuture<'a, ()> {
            self.attempted.push(channel.into());
            let outcome = if channel == self.reject {
                Err("bus refused".into())
            } else {
                Ok(())
            };
            Box::pin(async move { outcome })
        }
    }

    /// Baseline: rejected checkpoints stop before another phase can execute.
    #[test]
    fn baseline_checkpoint_delivery_failure_stops_the_run() {
        let mut bus = RejectingBus {
            reject: events::CHECKPOINT,
            attempted: Vec::new(),
        };
        let mut entry = EntryRecorder::default();
        let mut calls = Vec::new();
        let mut step = |phase: PhaseKind| {
            calls.push(phase.name());
            Ok(StepReport::default())
        };
        let error = ready(drive(
            &plan(&[PhaseKind::Recon, PhaseKind::Model]),
            &mut bus,
            &mut entry,
            &mut step,
        ))
        .unwrap_err();
        assert_eq!(error, "bus refused");
        assert_eq!(calls, ["recon"]);
        assert_eq!(
            bus.attempted,
            [events::STARTED, events::PHASE, events::CHECKPOINT]
        );
        assert_eq!(
            entry.bodies().last().expect("running entry")["status"],
            "running"
        );
    }

    /// An entry port refusing the first live run entry.
    struct RejectingEntry;
    impl SessionEntry for RejectingEntry {
        fn append(&mut self, _body: &Value) -> Result<(), String> {
            Err("entry refused".into())
        }
    }

    /// Baseline: a refused opening entry prevents all phase work.
    #[test]
    fn baseline_opening_entry_failure_prevents_phase_work() {
        let mut bus = BusRecorder::default();
        let mut called = false;
        let mut step = |_| {
            called = true;
            Ok(StepReport::default())
        };
        let error = ready(drive(
            &plan(&[PhaseKind::Recon]),
            &mut bus,
            &mut RejectingEntry,
            &mut step,
        ))
        .unwrap_err();
        assert_eq!(error, "entry refused");
        assert!(!called);
        assert_eq!(bus.channels(), [events::STARTED]);
    }

    /// Baseline: target validation fails before lifecycle events or session entries.
    #[test]
    fn baseline_empty_target_emits_no_lifecycle_events() {
        let mut bus = BusRecorder::default();
        let mut entry = EntryRecorder::default();
        let error = ready(super::execute(
            &mut bus,
            &mut entry,
            &crate::pipeline::AssessParams::default(),
            "run-blank",
        ))
        .unwrap_err();
        assert_eq!(error, "target is required");
        assert!(bus.channels().is_empty());
        assert!(entry.bodies().is_empty());
    }
}
