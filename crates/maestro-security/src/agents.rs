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
