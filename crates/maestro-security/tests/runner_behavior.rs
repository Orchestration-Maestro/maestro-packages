//! Baseline checks of image inventory and campaign failure handling.
#[cfg(test)]
pub mod support;

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
            (403, "runner forbidden".into()),
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
    assert_eq!(
        tally.failed,
        [
            "first.example: runner preflight failed: runner http 401 on /health",
            "second.example: runner preflight failed: runner http 403 on /health",
        ]
    );
    assert!(tally.assessments.is_empty());
    assert_eq!(tally.total_findings, 0);
    assert_eq!(tally.total_refuted, 0);
    assert_eq!(tally.budget.spent_tokens, 0);
    assert!(!tally.budget_warned);
}

/// An unavailable runner is an inspection error, not an empty image inventory.
#[test]
fn prestage_propagates_runner_inspection_failure() {
    if !support::runner(
        "prestage_propagates_runner_inspection_failure",
        vec![(401, "runner denied".into())],
    ) {
        return;
    }
    let error = prestage().expect_err("inspection must fail");
    assert!(error.contains("401"));
}

/// Baseline: a failed start keeps its failed state and committed revision.
#[test]
fn baseline_failed_start_retains_the_committed_workspace_revision() {
    if !support::runner(
        "baseline_failed_start_retains_the_committed_workspace_revision",
        vec![(401, "runner denied".into())],
    ) {
        return;
    }
    let error = maestro_security::workspace::dispatch(&maestro_security::workspace::Order {
        action: "start".into(),
        name: Some("review".into()),
        target: Some("example.test".into()),
        model: Some("fixture".into()),
        ..Default::default()
    })
    .expect_err("runner preflight must fail");
    assert!(error.contains("401"));
    let workspace = maestro_security::Store
        .get_workspace("review")
        .expect("workspace table")
        .expect("failed workspace");
    assert!(matches!(workspace.status, maestro_security::Status::Failed));
    assert_eq!(workspace.revision, 2);
    assert_eq!(workspace.target, "example.test");
    assert_eq!(workspace.model.as_deref(), Some("fixture"));
}

/// A successful start returns the revision retained by the following status read.
#[test]
fn start_response_revision_matches_the_retained_workspace() {
    use maestro_security::workspace::{Order, dispatch};

    support::isolated(
        "start_response_revision_matches_the_retained_workspace",
        "fixture-model",
        &[r#"{"services":[],"notes":"22/tcp open ssh"}"#],
        |_| {
            let params = Order {
                action: "start".into(),
                name: Some("review".into()),
                target: Some("example.test".into()),
                agent: Some("maestro.probe".into()),
                model: Some("fixture-model".into()),
                ..Default::default()
            };
            let started = dispatch(&params).expect("successful start");
            let workspace = started.workspace.expect("completed workspace");
            let retained = dispatch(&Order {
                action: "status".into(),
                ..params
            })
            .expect("status")
            .workspace
            .expect("retained workspace");
            assert!(matches!(
                workspace.status,
                maestro_security::Status::Completed
            ));
            assert_eq!(workspace.run_id, started.report.expect("report").run_id);
            assert_eq!(workspace.revision, retained.revision);
            assert_eq!(retained.revision, 2);
        },
    );
}
