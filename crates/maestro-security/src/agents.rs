//! The agent roster: five maestro-branded roles over one pipeline.
//!
//! An agent is a role prompt plus the phases it runs, in the pipeline's
//! canonical order. The engine is shared; only the role and the phase
//! scope differ.

use crate::pipeline::PhaseKind;

/// One agent role.
pub struct Agent {
    /// Public name, e.g. `maestro.security`.
    pub name: &'static str,
    /// One-line description for tool schemas and reports.
    pub description: &'static str,
    /// Role prompt appended to the system prompt for this agent's calls.
    pub role: &'static str,
    /// Phases this agent runs, in canonical order.
    pub phases: &'static [PhaseKind],
}

/// The operational pattern a run follows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Pattern {
    /// Offensive: find and prove, deepest first.
    #[default]
    RedTeam,
    /// Defensive: assess and harden, standard controls only.
    BlueTeam,
    /// Bounty: scope, deduplicate, disclose.
    BugBounter,
    /// Compliance: map findings to the standard's requirements.
    Compliance,
    /// Continuous operations: re-run, diff against the last run.
    ContinuousOps,
    /// Incident response: timeline, root cause, containment.
    Dfir,
}

impl Pattern {
    /// Every known pattern, in canonical order.
    pub const ALL: [Pattern; 6] = [
        Pattern::RedTeam,
        Pattern::BlueTeam,
        Pattern::BugBounter,
        Pattern::Compliance,
        Pattern::ContinuousOps,
        Pattern::Dfir,
    ];

    /// The wire name of the pattern.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Pattern::RedTeam => "red_team",
            Pattern::BlueTeam => "blue_team",
            Pattern::BugBounter => "bug_bounter",
            Pattern::Compliance => "compliance",
            Pattern::ContinuousOps => "continuous_ops",
            Pattern::Dfir => "dfir",
        }
    }

    /// Parse a wire name; an unknown name is the default pattern.
    #[must_use]
    pub fn parse(name: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|p| p.name() == name)
            .unwrap_or(Pattern::RedTeam)
    }

    /// The guidance line appended to the system prompt for the pattern.
    #[must_use]
    pub fn guidance(&self) -> &'static str {
        match self {
            Pattern::RedTeam => {
                "PATTERN red_team: find and prove. Order findings deepest \
first. An unproven candidate is needs_validation, never a guess."
            }
            Pattern::BlueTeam => {
                "PATTERN blue_team: assess and harden. Every fix is a \
standard, named control. Rank by risk reduced per unit of effort."
            }
            Pattern::BugBounter => {
                "PATTERN bug_bounter: scope first, deduplicate against \
known reports, then disclose. A finding outside scope is listed as out \
of scope, not dropped silently."
            }
            Pattern::Compliance => {
                "PATTERN compliance: map every finding to the standard's \
requirement it violates. A finding without a requirement mapping is an \
observation, not a compliance failure."
            }
            Pattern::ContinuousOps => {
                "PATTERN continuous_ops: this is a re-run. Diff against the \
previous run: new, changed, and resolved. Report the delta, not the whole."
            }
            Pattern::Dfir => {
                "PATTERN dfir: build the timeline first, then the root \
cause, then containment. Every timeline entry cites its evidence id."
            }
        }
    }
}

/// The three-agent panel over one finding: the finder proposes, the
/// verifier attacks it, the reporter writes it up. The panel is data, not
/// code: the same three roles for every finding.
pub struct FindingPanel {
    /// The role that found the candidate.
    pub finder: &'static str,
    /// The role that tries to refute it.
    pub verifier: &'static str,
    /// The role that writes the report entry.
    pub reporter: &'static str,
}

/// The default panel: probe finds, proof verifies, security reports.
pub const PANEL: FindingPanel = FindingPanel {
    finder: "maestro.probe",
    verifier: "maestro.proof",
    reporter: "maestro.security",
};

/// The full roster. `maestro.security` is the default.
pub const ROSTER: &[Agent] = &[
    Agent {
        name: "maestro.security",
        description: "Security reviewer: full seven-phase pipeline, findings with evidence, impact and fix.",
        role: "You are maestro.security, the security reviewer. You find \
weaknesses that could be exploited, so they can be fixed before shipping. \
Every claim cites the command that proves it \
and its evidence id. A confirmed finding with impact and fix beats ten \
guesses. When the output does not prove the claim, you say so.",
        phases: &[
            PhaseKind::Recon,
            PhaseKind::Model,
            PhaseKind::Hypotheses,
            PhaseKind::Verify,
            PhaseKind::Deepen,
            PhaseKind::Threat,
            PhaseKind::Report,
        ],
    },
    Agent {
        name: "maestro.probe",
        description: "Evidence collector: raw recon and the structured target model, every fact ledgered.",
        role: "You are maestro.probe, the evidence collector. You record \
what you see, not what you expect: services, ports, technologies, \
endpoints, identities. Every fact carries its evidence id. Anything you \
looked for but did not prove goes to UNKNOWN, never into the model.",
        phases: &[PhaseKind::Recon, PhaseKind::Model],
    },
    Agent {
        name: "maestro.proof",
        description: "Independent verifier: counterevidence per confirmation, UNKNOWN for incomplete evidence.",
        role: "You are maestro.proof, the independent verifier. You assume \
every hypothesis is wrong until the output proves it otherwise. For each \
confirmation you seek counterevidence: one command that would disprove \
the result. When the evidence is incomplete you mark the item UNKNOWN and \
say exactly what is missing. You never upgrade an evidence level to \
please the report.",
        phases: &[
            PhaseKind::Recon,
            PhaseKind::Model,
            PhaseKind::Hypotheses,
            PhaseKind::Verify,
        ],
    },
    Agent {
        name: "maestro.rebuild",
        description: "Reconstructor: rebuilds the target's architecture from verified evidence.",
        role: "You are maestro.rebuild, the reconstructor. From verified \
evidence you rebuild what the target is: its architecture, its trust \
boundaries, its data flows. You state only what the evidence supports. \
Observed mechanics that should not be imitated are listed separately. \
Gaps are named, not filled with guesses.",
        phases: &[PhaseKind::Recon, PhaseKind::Model, PhaseKind::Report],
    },
    Agent {
        name: "maestro.cyber",
        description: "Cyber operations: threat model and standard hardening from the target model.",
        role: "You are maestro.cyber, the cyber operations analyst. From \
the target model you build the threat model: assets and their value, the \
actors who want them, the exposure paths between them, and the minimal \
standard hardening for each path. You rank by impact times likelihood. \
Every hardening is a standard, named control, not a bespoke invention.",
        phases: &[
            PhaseKind::Recon,
            PhaseKind::Model,
            PhaseKind::Threat,
            PhaseKind::Report,
        ],
    },
];

/// Resolve an agent by name; an unknown or missing name is the default.
#[must_use]
pub fn resolve_agent(name: Option<&str>) -> &'static Agent {
    let default = &ROSTER[0];
    name.and_then(|n| ROSTER.iter().find(|a| a.name == n))
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    //! Unit tests for the roster and patterns.

    use super::{Pattern, PANEL, ROSTER};

    /// Unknown pattern names fall back to the default.
    #[test]
    fn unknown_pattern_is_red_team() {
        assert_eq!(Pattern::parse("bogus"), Pattern::RedTeam);
        assert_eq!(Pattern::parse("dfir"), Pattern::Dfir);
        assert_eq!(Pattern::ALL.len(), 6);
    }

    /// The panel names three distinct roster agents.
    #[test]
    fn panel_roles_are_roster_agents() {
        for name in [PANEL.finder, PANEL.verifier, PANEL.reporter] {
            assert!(ROSTER.iter().any(|a| a.name == name));
        }
    }
}
