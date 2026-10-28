//! Registered controls route to the addressed lifecycle, not another run.

use super::*;
use crate::autonomy::run::{
    Control, EventSink, Outcome, Plan, SessionEntry, StepReport, drive_controlled,
};

/// Successful host ports; assertions observe phase work and the terminal outcome.
struct Ports;

impl EventSink for Ports {
    fn emit<'a>(&'a mut self, _: &'a str, _: Value) -> ExtensionFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}

impl SessionEntry for Ports {
    fn append(&mut self, _: &Value) -> Result<(), String> {
        Ok(())
    }
}
use crate::pipeline::PhaseKind;

/// Drive one recon phase through a registered run's control handle.
fn controlled(id: &str, control: &Control, notes: &mut Vec<String>) -> Outcome {
    let plan = Plan {
        run_id: id,
        target: "example.test",
        phases: &[PhaseKind::Recon],
    };
    let mut step = |_, guidance: &[String]| {
        notes.extend_from_slice(guidance);
        Ok(StepReport::default())
    };
    ready(drive_controlled(
        &plan, &mut Ports, &mut Ports, control, &mut step,
    ))
    .expect("drive")
}

/// Commands steer and stop the named run while leaving another run alone.
#[test]
fn registered_controls_reach_only_the_addressed_run() {
    crate::test_support::isolated(|_| {
        let host = Rc::new(Host::default());
        ready(factory(ExtensionAPI::new(Rc::new(ApiHost(host.clone()))))).expect("register");
        let id = crate::autonomy::start_run("example.test").expect("start");
        let other = crate::autonomy::start_run("other.test").expect("start other");
        let control = crate::autonomy::steer::for_run(&id).expect("control");
        let steer = host.commands.borrow()["security-steer"].handler.clone();
        ready(steer(
            format!("{id} check the API"),
            ExtensionCommandContext::new(Rc::new(CommandContext)),
        ))
        .expect("steer");
        let mut notes = Vec::new();
        assert_eq!(controlled(&id, &control, &mut notes), Outcome::Completed);
        assert_eq!(notes, ["check the API"]);
        let stop = host.commands.borrow()["security-stop"].handler.clone();
        ready(stop(
            id.clone(),
            ExtensionCommandContext::new(Rc::new(CommandContext)),
        ))
        .expect("stop");
        notes.clear();
        assert_eq!(controlled(&id, &control, &mut notes), Outcome::Killed);
        assert!(notes.is_empty());
        let other_control = crate::autonomy::steer::for_run(&other).expect("other control");
        assert_eq!(
            controlled(&other, &other_control, &mut notes),
            Outcome::Completed
        );
        assert!(notes.is_empty());
        crate::autonomy::finish_run(&id, Err("test ended")).expect("settle");
        crate::autonomy::finish_run(&other, Err("test ended")).expect("settle other");
    });
}
