//! Multi-target orchestration: campaign assessment across many targets with
//! aggregate reporting, and goal-driven operation that plans the next action
//! from the results.

use serde::{Deserialize, Serialize};

use crate::pipeline::AssessParams;
use crate::report::Assessment;

/// Schema tag for the campaign report.
pub const SCHEMA: &str = "maestro-security/campaign/2";

/// A typed supervisor decision: the model proposes, deterministic code owns
/// the state. A stale `base_revision` means the decision was made against an
/// old state and must be re-derived, not applied.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    /// The state revision the decision was made against.
    pub base_revision: u64,
    /// New tasks to add, each citing the evidence that justifies it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub new_tasks: Vec<TaskProposal>,
    /// The id to assign to the next task.
    pub next_task_id: u32,
    /// True when the supervisor proposes to finish.
    pub finish: bool,
    /// The evidence ids that justify finishing; required when finishing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub finish_basis_ids: Vec<String>,
    /// One-line summary of the decision.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub summary: String,
}

/// One proposed task: the smallest evidence-backed step, nothing speculative.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskProposal {
    /// What to do, in one imperative sentence.
    pub task: String,
    /// The evidence ids that justify this task.
    pub basis_ids: Vec<String>,
}

impl Decision {
    /// A finish is only valid when it cites its evidence basis.
    #[must_use]
    pub fn finish_is_valid(&self) -> bool {
        !self.finish || !self.finish_basis_ids.is_empty()
    }
}

/// The budget of one run: hours and tokens, with a soft warning line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentBudget {
    /// Maximum agent hours for the run.
    pub max_hours: u32,
    /// Maximum output tokens for the run.
    pub max_tokens: u64,
    /// The token count at which a soft warning is raised.
    pub warn_at_tokens: u64,
    /// Output tokens spent so far.
    pub spent_tokens: u64,
}

impl AgentBudget {
    /// The default budget: 4 hours, 2 million tokens, warn at 80 percent.
    #[must_use]
    pub fn default_() -> Self {
        Self {
            max_hours: 4,
            max_tokens: 2_000_000,
            warn_at_tokens: 1_600_000,
            spent_tokens: 0,
        }
    }

    /// The unspent token budget, floored at zero.
    #[must_use]
    pub fn remaining(&self) -> u64 {
        self.max_tokens.saturating_sub(self.spent_tokens)
    }

    /// True once the soft warning line is crossed.
    #[must_use]
    pub fn should_warn(&self) -> bool {
        self.spent_tokens >= self.warn_at_tokens
    }

    /// True when the hard token limit is reached.
    #[must_use]
    pub fn exhausted(&self) -> bool {
        self.spent_tokens >= self.max_tokens
    }

    /// Record one run's spend against the budget.
    pub fn spend(&mut self, tokens: u64) {
        self.spent_tokens = self.spent_tokens.saturating_add(tokens);
    }
}

/// Parameters for a multi-target campaign.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Targets {
    /// Targets to assess, in order.
    pub targets: Vec<String>,
    /// Agent role (default `maestro.security`).
    pub agent: Option<String>,
    /// Model override.
    pub model: Option<String>,
    /// Max hypotheses per target.
    pub max_hypotheses: Option<usize>,
}

/// The aggregated result of a campaign run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tally {
    /// Schema tag.
    pub schema: &'static str,
    /// The targets, in input order.
    pub targets: Vec<String>,
    /// The per-target assessments that succeeded.
    pub assessments: Vec<Assessment>,
    /// Total findings across all targets.
    pub total_findings: usize,
    /// Total refuted hypotheses across all targets.
    pub total_refuted: usize,
    /// Targets that failed, as `target: error` strings.
    pub failed: Vec<String>,
    /// The budget state at the end of the campaign.
    pub budget: AgentBudget,
    /// True when the soft warning line was crossed during the campaign.
    pub budget_warned: bool,
    /// One-line summary.
    pub summary: String,
}

/// Run the pipeline across every target and aggregate the results. A failing
/// target is recorded in `failed` and does not stop the campaign.
///
/// # Errors
/// Fails if no targets are given.
pub fn run(params: &Targets) -> Result<Tally, String> {
    if params.targets.is_empty() {
        return Err("at least one target is required".into());
    }
    let mut assessments = Vec::new();
    let mut failed = Vec::new();
    let mut budget = AgentBudget::default_();
    let mut budget_warned = false;
    for target in &params.targets {
        if budget.exhausted() {
            failed.push(format!("{target}: budget exhausted"));
            continue;
        }
        let assess = AssessParams {
            target: target.clone(),
            agent: params.agent.clone(),
            model: params.model.clone(),
            max_hypotheses: params.max_hypotheses,
            pattern: None,
        };
        match crate::pipeline::assess(&assess) {
            Ok(report) => {
                let tokens = report.spend.as_ref().map_or(0, |s| s.tokens_out);
                budget.spend(tokens);
                budget_warned |= budget.should_warn();
                assessments.push(report);
            }
            Err(error) => failed.push(format!("{target}: {error}")),
        }
    }
    let (total_findings, total_refuted) = campaign_totals(&assessments);
    let summary = format!(
        "Campaign over {} targets: {} findings, {} refuted, {} failed.{}",
        params.targets.len(),
        total_findings,
        total_refuted,
        failed.len(),
        if budget_warned { " budget warning raised" } else { "" }
    );
    Ok(Tally {
        schema: SCHEMA,
        targets: params.targets.clone(),
        assessments,
        total_findings,
        total_refuted,
        failed,
        budget,
        budget_warned,
        summary,
    })
}

/// The campaign totals across the successful assessments.
fn campaign_totals(assessments: &[Assessment]) -> (usize, usize) {
    (
        assessments.iter().map(|a| a.findings.len()).sum(),
        assessments.iter().map(|a| a.refuted.len()).sum(),
    )
}

#[cfg(test)]
mod tests {
    //! Unit tests for campaign aggregation.

    use super::{run, AgentBudget, Decision, Targets};

    /// An empty target list is rejected before any pipeline work.
    #[test]
    fn empty_campaign_is_rejected() {
        let err = run(&Targets::default()).unwrap_err();
        assert!(err.contains("target"));
    }

    /// A finish without a basis is invalid; one with is valid.
    #[test]
    fn finish_requires_a_basis() {
        let mut d = Decision {
            base_revision: 1,
            new_tasks: Vec::new(),
            next_task_id: 2,
            finish: true,
            finish_basis_ids: Vec::new(),
            summary: String::new(),
        };
        assert!(!d.finish_is_valid());
        d.finish_basis_ids.push("ev-0001".into());
        assert!(d.finish_is_valid());
    }

    /// The budget warns at the soft line and exhausts at the hard one.
    #[test]
    fn budget_warns_and_exhausts() {
        let mut b = AgentBudget {
            max_tokens: 100,
            warn_at_tokens: 80,
            ..AgentBudget::default_()
        };
        b.spend(50);
        assert!(!b.should_warn());
        assert_eq!(b.remaining(), 50);
        b.spend(30);
        assert!(b.should_warn());
        b.spend(20);
        assert!(b.exhausted());
        assert_eq!(b.remaining(), 0);
    }
}
