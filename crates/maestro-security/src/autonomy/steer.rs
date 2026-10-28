//! Shared stop signal and bounded phase-boundary guidance.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};

/// A run's control handle, shared with its command callers.
#[derive(Clone, Default)]
pub struct Control(Arc<Mutex<Pending>>);

/// Stop signal and the five most recent unconsumed messages.
#[derive(Default)]
struct Pending {
    /// Stop requested by a caller.
    stopped: bool,
    /// Messages awaiting delivery to the lifecycle.
    notes: VecDeque<(String, String)>,
}

impl Control {
    /// Request a kill at the next phase boundary.
    pub fn stop(&self) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stopped = true;
    }

    /// Queue guidance; retain at most five pending messages, dropping oldest.
    pub fn steer(&self, source: &str, message: &str) {
        let mut pending = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if pending.notes.len() == 5 {
            pending.notes.pop_front();
        }
        pending.notes.push_back((source.into(), message.into()));
    }

    /// Whether a caller has requested a kill.
    pub(crate) fn stopped(&self) -> bool {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stopped
    }

    /// Take guidance awaiting a phase boundary.
    pub(crate) fn take(&self) -> Vec<(String, String)> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .notes
            .drain(..)
            .collect()
    }
}

/// Controls owned by the current extension process, keyed by run identity.
fn registry() -> &'static Mutex<BTreeMap<String, Control>> {
    static RUNS: OnceLock<Mutex<BTreeMap<String, Control>>> = OnceLock::new();
    RUNS.get_or_init(Mutex::default)
}

/// Make a newly recorded run addressable by control commands.
pub(super) fn register(run_id: &str) {
    registry()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(run_id.into(), Control::default());
}

/// Find a live run's control handle.
///
/// # Errors
/// Rejects unknown runs and runs whose lifecycle has already settled.
pub fn for_run(run_id: &str) -> Result<Control, String> {
    registry()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(run_id)
        .cloned()
        .ok_or_else(|| format!("no live run {run_id}"))
}

/// Remove a settled run from command routing.
pub(super) fn remove(run_id: &str) {
    registry()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(run_id);
}
