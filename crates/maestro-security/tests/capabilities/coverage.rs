//! Baseline coverage-ledger behaviours through exported types and methods.

use maestro_security::{Attempt, CoverageDoc, CoverageStatus};

/// Plan a single input-boundary unit for each independent ledger scenario.
fn planned() -> CoverageDoc {
    let mut doc = CoverageDoc::new("fixture-host");
    doc.plan("web", "input", "login", "injection");
    doc
}

/// Baseline: planning deduplicates tuples without replacing completed work.
#[test]
fn planning_preserves_existing_work_and_distinguishes_tuples() {
    let mut doc = planned();
    let id = CoverageDoc::unit_id("web", "input", "login", "injection");
    assert!(doc.set_status(&id, CoverageStatus::Deferred, "requires credentials"));
    doc.plan("web", "input", "login", "injection");
    doc.plan("api", "input", "login", "injection");
    assert_eq!(doc.units.len(), 2);
    assert_eq!(doc.units[0].status, CoverageStatus::Deferred);
    assert_eq!(doc.units[0].reason, "requires credentials");
    assert_eq!(doc.units[1].status, CoverageStatus::Planned);
    assert!(doc.violations().is_empty());
    assert!(!doc.set_status("absent", CoverageStatus::Covered, ""));
    assert!(!doc.attempt("absent", "probe", vec![], "not run"));
    assert_eq!(doc.units.len(), 2);
}

/// Baseline: exclusions need a reason and may not silently carry performed work.
#[test]
fn exclusions_require_reasons_and_reject_work() {
    for status in [
        CoverageStatus::Planned,
        CoverageStatus::NotApplicable,
        CoverageStatus::OutOfScope,
    ] {
        let mut doc = planned();
        let id = doc.units[0].coverage_id.clone();
        assert!(doc.set_status(&id, status, ""));
        assert_eq!(
            doc.violations().len(),
            usize::from(status != CoverageStatus::Planned)
        );
        assert!(doc.set_status(&id, status, "not exposed"));
        assert!(doc.violations().is_empty());
        doc.units[0].reviewed_paths.push("src/login.rs".into());
        assert_eq!(doc.violations().len(), 1);
        doc.units[0].reviewed_paths.clear();
        doc.units[0].local_checks.push("input replay".into());
        assert_eq!(doc.violations().len(), 1);
        doc.units[0].local_checks.clear();
        doc.units[0]
            .result_fingerprints
            .push("login-injection".into());
        assert_eq!(doc.violations().len(), 1);
    }
}

/// Baseline: each active or terminal state requires its appropriate evidence.
#[test]
fn statuses_require_owner_checks_findings_blockers_or_reason() {
    for status in [
        CoverageStatus::InProgress,
        CoverageStatus::Covered,
        CoverageStatus::Candidate,
        CoverageStatus::Blocked,
        CoverageStatus::Deferred,
    ] {
        let mut doc = planned();
        doc.units[0].status = status;
        assert_eq!(doc.violations().len(), 1, "{status:?}");
        let id = doc.units[0].coverage_id.clone();
        match status {
            CoverageStatus::InProgress => assert!(doc.set_status(&id, status, "")),
            CoverageStatus::Covered => doc.units[0].local_checks.push("replay".into()),
            CoverageStatus::Candidate => doc.units[0]
                .result_fingerprints
                .push("login-injection".into()),
            CoverageStatus::Blocked => doc.units[0].unresolved.push("credentials".into()),
            CoverageStatus::Deferred => assert!(doc.set_status(&id, status, "owner scheduled")),
            _ => unreachable!(),
        }
        assert!(doc.violations().is_empty(), "{status:?}");
    }
    let mut doc = planned();
    doc.units[0].status = CoverageStatus::Covered;
    doc.units[0]
        .result_fingerprints
        .push("login-injection".into());
    assert!(doc.violations().is_empty());
}

/// Baseline: attempts retain evidence, reject repeat owners and validate imports.
#[test]
fn attempts_preserve_history_and_detect_invalid_imports() {
    let mut doc = planned();
    let id = doc.units[0].coverage_id.clone();
    assert!(doc.attempt(&id, "probe", vec!["replay".into()], "blocked"));
    assert!(!doc.attempt(&id, "probe", vec!["discarded".into()], "repeat"));
    assert!(doc.attempt(
        &id,
        "proof",
        vec!["authenticated replay".into()],
        "confirmed"
    ));
    let attempts = &doc.units[0].attempts;
    assert_eq!(attempts.len(), 2);
    assert_eq!((attempts[0].wave, attempts[1].wave), (1, 2));
    assert_eq!(attempts[0].checks, ["replay"]);
    assert_eq!(attempts[1].outcome, "confirmed");
    assert!(doc.violations().is_empty());
    doc.units[0].attempts.push(Attempt {
        wave: 4,
        owner: "other".into(),
        checks: vec![],
        outcome: String::new(),
    });
    assert_eq!(doc.violations().len(), 1);
    doc.units[0].attempts[2].wave = 3;
    doc.units[0].attempts[2].owner = "proof".into();
    assert_eq!(doc.violations().len(), 1);
    doc.units[0].attempts[2].owner = "other".into();
    assert!(doc.violations().is_empty());
    doc.units.push(doc.units[0].clone());
    assert_eq!(doc.violations().len(), 1);
}
