//! Baseline checks of image inventory and campaign failure handling.
mod support;

use maestro_security::prestage::prestage;

/// Baseline: exact image references count once; near matches do not count.
#[test]
fn baseline_prestage_counts_exact_image_matches() {
    if !support::runner(
        "baseline_prestage_counts_exact_image_matches",
        vec![support::output(
            " docker.io/library/alpine:3.20 \ndocker.io/library/alpine:3.20\ndocker.io/library/python:3.12-slim\ndocker.io/library/node:22-alpine-extra\n",
        )],
    ) {
        return;
    }
    let report = prestage().expect("prestage");
    assert_eq!(report.present, 2);
    assert_eq!(report.missing, 1);
    assert_eq!(report.images.len(), 3);
    assert_eq!(report.images[0].status, "present");
    assert_eq!(report.images[1].status, "present");
    assert_eq!(
        report.images[2].reference,
        "docker.io/library/node:22-alpine"
    );
    assert_eq!(report.images[2].status, "missing");
}

/// Baseline: an empty runner listing reports every required image as missing.
#[test]
fn baseline_prestage_reports_an_empty_inventory() {
    if !support::runner(
        "baseline_prestage_reports_an_empty_inventory",
        vec![support::output("")],
    ) {
        return;
    }
    let report = prestage().expect("prestage");
    assert_eq!(report.present, 0);
    assert_eq!(report.missing, 3);
    assert!(report.images.iter().all(|image| image.status == "missing"));
}

/// Baseline: a failing campaign target cannot prevent later targets being attempted.
#[test]
fn baseline_campaign_collects_each_target_failure_in_input_order() {
    if !support::runner(
        "baseline_campaign_collects_each_target_failure_in_input_order",
        vec![
            (401, "runner unavailable".into()),
            (401, "runner unavailable".into()),
        ],
    ) {
        return;
    }
    let targets = vec!["first.example".into(), "second.example".into()];
    let tally = maestro_security::campaign::run(&maestro_security::campaign::Targets {
        targets: targets.clone(),
        model: Some("fixture".into()),
        ..Default::default()
    })
    .expect("campaign tally");
    assert_eq!(tally.targets, targets);
    assert_eq!(tally.failed.len(), 2);
    assert!(tally.failed[0].starts_with("first.example:"));
    assert!(tally.failed[1].starts_with("second.example:"));
    assert!(
        tally
            .failed
            .iter()
            .all(|failure| failure.contains("runner preflight"))
    );
    assert!(tally.assessments.is_empty());
    assert_eq!(tally.total_findings, 0);
    assert_eq!(tally.total_refuted, 0);
    assert_eq!(tally.budget.spent_tokens, 0);
    assert!(!tally.budget_warned);
}
