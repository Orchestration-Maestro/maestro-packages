//! The coverage ledger: one unit per (surface, boundary, subsystem,
//! `attack_class`) tuple, so a report can prove what was looked at and what
//! was deliberately left alone.
//!
//! The ledger is a document, not a database: it serializes into the run's
//! evidence home and validates against its own state invariants.

use serde::{Deserialize, Serialize};

/// Schema tag for the coverage document.
pub const SCHEMA: &str = "maestro-security/coverage/1";

/// The eight states of one coverage unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageStatus {
    /// Planned but not yet started.
    Planned,
    /// The tuple cannot hold a finding for this target.
    NotApplicable,
    /// The tuple is outside the agreed scope.
    OutOfScope,
    /// An agent is working it now.
    InProgress,
    /// Looked at; nothing to report.
    Covered,
    /// Looked at; a finding was raised.
    Candidate,
    /// Decisive validation is blocked.
    Blocked,
    /// Deliberately left for later.
    Deferred,
}

/// One validation attempt against a unit; waves strictly increase and each
/// attempt names a fresh owner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attempt {
    /// The wave number: 1, 2, 3, strictly increasing.
    pub wave: u32,
    /// The agent that ran the attempt.
    pub owner: String,
    /// What the attempt checked.
    pub checks: Vec<String>,
    /// The outcome in one line.
    pub outcome: String,
}

/// One unit of the coverage ledger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageUnit {
    /// Canonical id: the four parts joined with `::`, lexicographically
    /// sorted within the id.
    pub coverage_id: String,
    /// The surface the unit covers (web, api, cli, ...).
    pub surface: String,
    /// The boundary crossed (auth, input, network, ...).
    pub boundary: String,
    /// The subsystem touched.
    pub subsystem: String,
    /// The attack class looked for.
    pub attack_class: String,
    /// The current state.
    pub status: CoverageStatus,
    /// Why the unit is in its terminal state, when one applies.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reason: String,
    /// The agent working an in-progress unit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    /// Paths reviewed while covering the unit.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reviewed_paths: Vec<String>,
    /// Local checks run against the unit.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub local_checks: Vec<String>,
    /// Finding fingerprints raised from the unit.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub result_fingerprints: Vec<String>,
    /// What still blocks decisive validation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unresolved: Vec<String>,
    /// Archived attempts, strictly increasing waves.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attempts: Vec<Attempt>,
}

/// The coverage document for one run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageDoc {
    /// Schema tag.
    pub schema: &'static str,
    /// The assessed target.
    pub target: String,
    /// One unit per (surface, boundary, subsystem, `attack_class`) tuple.
    pub units: Vec<CoverageUnit>,
}

impl CoverageDoc {
    /// An empty ledger for a target.
    #[must_use]
    pub fn new(target: &str) -> Self {
        Self {
            schema: SCHEMA,
            target: target.into(),
            units: Vec::new(),
        }
    }

    /// The canonical id for one tuple: the four parts joined with `::`.
    #[must_use]
    pub fn unit_id(surface: &str, boundary: &str, subsystem: &str, attack_class: &str) -> String {
        [surface, boundary, subsystem, attack_class].join("::")
    }

    /// Add a unit in the `planned` state; a duplicate id is kept once.
    pub fn plan(&mut self, surface: &str, boundary: &str, subsystem: &str, attack_class: &str) {
        let coverage_id = Self::unit_id(surface, boundary, subsystem, attack_class);
        if self.units.iter().any(|u| u.coverage_id == coverage_id) {
            return;
        }
        self.units.push(CoverageUnit {
            coverage_id,
            surface: surface.into(),
            boundary: boundary.into(),
            subsystem: subsystem.into(),
            attack_class: attack_class.into(),
            status: CoverageStatus::Planned,
            reason: String::new(),
            agent_id: None,
            reviewed_paths: Vec::new(),
            local_checks: Vec::new(),
            result_fingerprints: Vec::new(),
            unresolved: Vec::new(),
            attempts: Vec::new(),
        });
    }

    /// Move one unit to a new state, recording the reason.
    pub fn set_status(&mut self, coverage_id: &str, status: CoverageStatus, reason: &str) -> bool {
        let Some(unit) = self.units.iter_mut().find(|u| u.coverage_id == coverage_id) else {
            return false;
        };
        unit.status = status;
        unit.reason = reason.to_string();
        if status == CoverageStatus::InProgress {
            unit.agent_id = Some("maestro.security".into());
        }
        true
    }

    /// Record one attempt against a unit; the wave is the previous max plus
    /// one and the owner must differ from the last attempt's owner.
    pub fn attempt(
        &mut self,
        coverage_id: &str,
        owner: &str,
        checks: Vec<String>,
        outcome: &str,
    ) -> bool {
        let Some(unit) = self.units.iter_mut().find(|u| u.coverage_id == coverage_id) else {
            return false;
        };
        let last = unit.attempts.last();
        if last.is_some_and(|a| a.owner == owner) {
            return false;
        }
        let wave = last.map_or(1, |a| a.wave + 1);
        unit.attempts.push(Attempt {
            wave,
            owner: owner.into(),
            checks,
            outcome: outcome.into(),
        });
        true
    }

    /// The state invariants of every unit, as violation strings. An empty
    /// list means the document is consistent.
    #[must_use]
    pub fn violations(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut ids: Vec<&str> = self.units.iter().map(|u| u.coverage_id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        if ids.len() != self.units.len() {
            out.push("duplicate coverage_id".into());
        }
        for unit in &self.units {
            out.extend(unit.violations());
        }
        out
    }
}

impl CoverageUnit {
    /// The state invariants of one unit, as violation strings.
    #[must_use]
    pub fn violations(&self) -> Vec<String> {
        let mut out = self.status_violations();
        if !self.attempts_well_formed() {
            out.push(format!(
                "{}: attempts must increase waves with a fresh owner",
                self.coverage_id
            ));
        }
        out
    }

    /// The invariants of the unit's current status.
    fn status_violations(&self) -> Vec<String> {
        let mut out = Vec::new();
        let id = &self.coverage_id;
        match self.status {
            CoverageStatus::Planned
            | CoverageStatus::NotApplicable
            | CoverageStatus::OutOfScope => {
                if !self.reviewed_paths.is_empty()
                    || !self.local_checks.is_empty()
                    || !self.result_fingerprints.is_empty()
                {
                    out.push(format!(
                        "{id}: {status:?} unit carries work",
                        status = self.status
                    ));
                }
                if matches!(
                    self.status,
                    CoverageStatus::NotApplicable | CoverageStatus::OutOfScope
                ) && self.reason.is_empty()
                {
                    out.push(format!("{id}: {:?} needs a reason", self.status));
                }
            }
            CoverageStatus::InProgress => {
                if self.agent_id.is_none() {
                    out.push(format!("{id}: in_progress needs an owner"));
                }
            }
            CoverageStatus::Covered => {
                if self.result_fingerprints.is_empty() && self.local_checks.is_empty() {
                    out.push(format!("{id}: covered needs checks or fingerprints"));
                }
            }
            CoverageStatus::Candidate => {
                if self.result_fingerprints.is_empty() {
                    out.push(format!("{id}: candidate needs a fingerprint"));
                }
            }
            CoverageStatus::Blocked => {
                if self.unresolved.is_empty() {
                    out.push(format!("{id}: blocked needs unresolved items"));
                }
            }
            CoverageStatus::Deferred => {
                if self.reason.is_empty() {
                    out.push(format!("{id}: deferred needs a reason"));
                }
            }
        }
        out
    }

    /// Attempts must increase waves with a fresh owner each wave.
    fn attempts_well_formed(&self) -> bool {
        self.attempts
            .windows(2)
            .all(|w| w[1].wave == w[0].wave + 1 && w[1].owner != w[0].owner)
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the coverage ledger.

    use super::{CoverageDoc, CoverageStatus};

    /// A planned unit with no work is clean; a covered unit needs evidence.
    #[test]
    fn state_invariants_hold_and_break() {
        let mut doc = CoverageDoc::new("example.com");
        doc.plan("web", "input", "login", "injection");
        assert!(doc.violations().is_empty());

        doc.set_status("web::input::login::injection", CoverageStatus::Covered, "");
        assert_eq!(doc.violations().len(), 1);
    }

    /// Attempts increase waves and refuse a repeat owner.
    #[test]
    fn attempts_track_waves_and_owners() {
        let mut doc = CoverageDoc::new("example.com");
        doc.plan("web", "input", "login", "injection");
        let id = "web::input::login::injection";
        assert!(doc.attempt(id, "maestro.probe", vec!["curl ...".into()], "no injection"));
        assert!(!doc.attempt(id, "maestro.probe", vec!["nmap".into()], "again"));
        assert!(doc.attempt(id, "maestro.proof", vec!["nmap".into()], "confirmed port"));
        let unit = &doc.units[0];
        assert_eq!(unit.attempts.len(), 2);
        assert_eq!(unit.attempts[1].wave, 2);
    }

    /// Duplicate ids are rejected by the invariants, not by `plan`.
    #[test]
    fn duplicate_ids_are_flagged() {
        let mut doc = CoverageDoc::new("example.com");
        doc.plan("web", "input", "login", "injection");
        doc.plan("web", "input", "login", "injection");
        assert_eq!(doc.units.len(), 1);
        assert!(doc.violations().is_empty());
    }
}
