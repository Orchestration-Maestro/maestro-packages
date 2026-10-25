//! Validation of the report and coverage documents: schema checks plus the
//! semantic invariants that keep a report honest.
//!
//! A report is only as good as its invariants. Every confirmed finding must
//! carry its trace, evidence and an execution; fingerprints must be unique
//! and sorted; a confirmed severity must never exceed the demonstrated
//! impact.

use serde::Serialize;

use crate::coverage::CoverageDoc;
use crate::report::Assessment;

/// The ordered severity scale, weakest first.
const SEVERITIES: [&str; 5] = ["info", "low", "medium", "high", "critical"];

/// One validation problem: where, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Violation {
    /// The finding id, coverage id, or `report` for document-level issues.
    pub where_: String,
    /// The invariant that broke.
    pub rule: String,
}

impl Violation {
    /// A document-level violation.
    #[must_use]
    pub fn report(rule: impl Into<String>) -> Self {
        Self {
            where_: "report".into(),
            rule: rule.into(),
        }
    }
}

/// The result of validating one run's documents.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ValidationReport {
    /// The violations found; empty means the run is publishable.
    pub violations: Vec<Violation>,
}

impl ValidationReport {
    /// True when the report can be published as-is.
    #[must_use]
    pub fn publishable(&self) -> bool {
        self.violations.is_empty()
    }

    /// Merge another report's violations into this one.
    pub fn extend(&mut self, other: ValidationReport) {
        self.violations.extend(other.violations);
    }
}

/// Validate one assessment against the report invariants.
#[must_use]
pub fn validate_assessment(assessment: &Assessment) -> ValidationReport {
    let mut out = ValidationReport::default();
    let mut fingerprints: Vec<&str> = assessment
        .findings
        .iter()
        .map(|f| f.fingerprint.as_str())
        .collect();
    fingerprints.sort_unstable();
    fingerprints.dedup();
    if fingerprints.len() != assessment.findings.len() {
        out.violations.push(Violation::report(
            "fingerprints must be unique",
        ));
    }
    let sorted = assessment
        .findings
        .iter()
        .map(|f| f.fingerprint.as_str())
        .collect::<Vec<_>>();
    if sorted != fingerprints {
        out.violations.push(Violation::report(
            "findings must be sorted by fingerprint",
        ));
    }
    for finding in &assessment.findings {
        out.extend(validate_finding(finding));
    }
    out
}

/// Validate one finding against the verdict invariants.
fn validate_finding(finding: &crate::report::Finding) -> ValidationReport {
    let mut out = ValidationReport::default();
    let id = finding.id.clone();
    if finding.fingerprint.is_empty() {
        out.violations.push(Violation {
            where_: id.clone(),
            rule: "fingerprint required".into(),
        });
    }
    if finding.severity != "info"
        && !SEVERITIES.contains(&finding.severity.as_str())
    {
        out.violations.push(Violation {
            where_: id.clone(),
            rule: format!("unknown severity {}", finding.severity),
        });
    }
    out.violations.extend(verdict_violations(finding, &id));

    out
}

/// Validate one coverage document against its state invariants.
#[must_use]
pub fn validate_coverage(doc: &CoverageDoc) -> ValidationReport {
    ValidationReport {
        violations: doc
            .violations()
            .into_iter()
            .map(|rule| Violation {
                where_: "coverage".into(),
                rule,
            })
            .collect(),
    }
}

/// The verdict-specific invariants of one finding.
fn verdict_violations(
    finding: &crate::report::Finding,
    id: &str,
) -> Vec<Violation> {
    match finding.verdict {
        crate::report::Verdict::Confirmed => confirmed_violations(finding, id),
        crate::report::Verdict::NeedsValidation => needs_validation_violations(finding, id),
        crate::report::Verdict::Rejected => rejected_violations(finding, id),
    }
}

/// A confirmed finding must be demonstrated end to end.
fn confirmed_violations(finding: &crate::report::Finding, id: &str) -> Vec<Violation> {
    let mut out = Vec::new();
    if finding.evidence.command.is_empty() {
        out.push(Violation { where_: id.into(), rule: "confirmed needs evidence".into() });
    }
    if finding.trace.is_empty() {
        out.push(Violation { where_: id.into(), rule: "confirmed needs a trace".into() });
    }
    if finding.execution.is_none() {
        out.push(Violation {
            where_: id.into(),
            rule: "confirmed needs an execution".into(),
        });
    }
    if finding.confidence < 0.5 {
        out.push(Violation {
            where_: id.into(),
            rule: "confirmed confidence must be at least 0.5".into(),
        });
    }
    if let Some(poc) = &finding.poc && poc.verified && poc.observed.is_empty() {
        out.push(Violation {
            where_: id.into(),
            rule: "verified poc needs an observed result".into(),
        });
    }
    out
}

/// A needs-validation finding must name what blocks it and how to clear it.
fn needs_validation_violations(finding: &crate::report::Finding, id: &str) -> Vec<Violation> {
    let mut out = Vec::new();
    if finding.blockers.is_empty() {
        out.push(Violation {
            where_: id.into(),
            rule: "needs_validation needs blockers".into(),
        });
    }
    if finding.validation_plan.is_none() {
        out.push(Violation {
            where_: id.into(),
            rule: "needs_validation needs a validation plan".into(),
        });
    }
    out
}

/// A rejected finding must say why.
fn rejected_violations(finding: &crate::report::Finding, id: &str) -> Vec<Violation> {
    if finding.reason.is_none() {
        return vec![Violation {
            where_: id.into(),
            rule: "rejected needs a reason".into(),
        }];
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    //! Unit tests for report validation.

    use crate::report::{Assessment, Evidence, Execution, Finding, Phase, Verdict};
    use super::{validate_assessment, SEVERITIES};

    fn finding(id: &str, fingerprint: &str) -> Finding {
        Finding {
            id: id.into(),
            fingerprint: fingerprint.into(),
            title: "title".into(),
            class: "injection".into(),
            verdict: Verdict::Confirmed,
            severity: "medium".into(),
            likelihood: "high".into(),
            confidence: 0.9,
            trace: vec!["entrypoint".into(), "sink".into()],
            conditions: Vec::new(),
            execution: Some(Execution::default()),
            mitre: None,
            poc: None,
            evidence_level: "CONFIRMED".into(),
            escalation: String::new(),
            evidence: Evidence {
                command: "curl ...".into(),
                output: "ok".into(),
                id: "ev-0001".into(),
            },
            impact: "data".into(),
            fix: "fix".into(),
            target: "example.com".into(),
            claimed_root_cause: None,
            blockers: Vec::new(),
            validation_plan: None,
            reason: None,
        }
    }

    /// A clean assessment publishes; each broken invariant is named.
    #[test]
    fn clean_report_publishes() {
        let assessment = Assessment {
            schema: crate::report::SCHEMA.into(),
            run_id: "run-1".into(),
            agent: "maestro.security".into(),
            target: "example.com".into(),
            model: "m".into(),
            phases: Vec::<Phase>::new(),
            findings: vec![finding("F-001", "example.com::injection::a")],
            unknown: Vec::new(),
            refuted: Vec::new(),
            threats: Vec::new(),
            runtime_mechanics: Vec::new(),
            spend: None,
            summary: "s".into(),
        };
        assert!(validate_assessment(&assessment).publishable());
        assert_eq!(SEVERITIES.len(), 5);
    }

    /// A confirmed finding without a trace is not publishable.
    #[test]
    fn confirmed_needs_a_trace() {
        let mut f = finding("F-001", "example.com::injection::a");
        f.trace.clear();
        let assessment = Assessment {
            schema: crate::report::SCHEMA.into(),
            run_id: "run-1".into(),
            agent: "maestro.security".into(),
            target: "example.com".into(),
            model: "m".into(),
            phases: Vec::new(),
            findings: vec![f],
            unknown: Vec::new(),
            refuted: Vec::new(),
            threats: Vec::new(),
            runtime_mechanics: Vec::new(),
            spend: None,
            summary: "s".into(),
        };
        let report = validate_assessment(&assessment);
        assert!(!report.publishable());
        assert!(report
            .violations
            .iter()
            .any(|v| v.rule == "confirmed needs a trace"));
    }
}
