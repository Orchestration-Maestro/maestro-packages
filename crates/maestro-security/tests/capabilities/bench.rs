//! Baseline benchmark behaviours exercised through the public entry point.

use crate::support;

use maestro_security::bench::{Suite, bench};
use serde_json::Value;

/// Grade one answer and leave malformed or non-boolean grades uncredited.
fn graded(path: &str, body: &Value) -> (u16, String) {
    if path == "/models" {
        return support::models();
    }
    let system = body["messages"][0]["content"].as_str().unwrap_or("");
    let user = body["messages"][1]["content"].as_str().unwrap_or("");
    if system == "You are the grader." {
        if user.contains("HTTPS") {
            support::completion("```json\n{\"correct\":true}\n```")
        } else if user.contains("SQL") {
            support::completion("not JSON")
        } else {
            support::completion("{\"correct\":\"true\"}")
        }
    } else {
        support::completion(body["model"].as_str().unwrap_or(""))
    }
}

/// Baseline: A/B scores retain answers and count only affirmative boolean grades.
#[test]
fn comparison_counts_grades_and_preserves_answers() -> Result<(), Box<dyn std::error::Error>> {
    if !support::isolated(
        "bench::comparison_counts_grades_and_preserves_answers",
        graded,
    )? {
        return Ok(());
    }
    let report = bench(&Suite {
        model: "primary".into(),
        compare: Some("secondary".into()),
        judge: None,
    })?;
    assert_eq!(report.schema, "maestro-security/bench/1");
    let comparison = report.compare.ok_or("comparison missing")?;
    for (score, model) in [(&report.primary, "primary"), (&comparison, "secondary")] {
        assert_eq!(score.model, model);
        assert_eq!((score.correct, score.total), (1, 3));
        assert_eq!(score.answers.len(), score.total);
        assert!(score.answers.iter().all(|answer| answer.answer == model));
        assert_eq!(
            score.answers.iter().filter(|answer| answer.correct).count(),
            1
        );
    }
    assert_eq!(
        report
            .primary
            .answers
            .iter()
            .map(|a| &a.prompt)
            .collect::<Vec<_>>(),
        comparison
            .answers
            .iter()
            .map(|a| &a.prompt)
            .collect::<Vec<_>>()
    );
    Ok(())
}

/// Baseline: single-model runs omit comparison results.
#[test]
fn single_model_has_no_comparison() -> Result<(), Box<dyn std::error::Error>> {
    if !support::isolated("bench::single_model_has_no_comparison", graded)? {
        return Ok(());
    }
    let report = bench(&Suite {
        model: "primary".into(),
        ..Suite::default()
    })?;
    assert!(report.compare.is_none());
    assert_eq!((report.primary.correct, report.primary.total), (1, 3));
    Ok(())
}

/// Deny completions without triggering transient HTTP retries.
fn denied(path: &str, _: &Value) -> (u16, String) {
    if path == "/models" {
        support::models()
    } else {
        (403, "denied".into())
    }
}

/// Baseline: model errors abort rather than producing a misleading scorecard.
#[test]
fn completion_failure_is_returned() -> Result<(), Box<dyn std::error::Error>> {
    if !support::isolated("bench::completion_failure_is_returned", denied)? {
        return Ok(());
    }
    let error = bench(&Suite {
        model: "primary".into(),
        ..Suite::default()
    })
    .unwrap_err();
    assert!(error.contains("403"));
    assert!(error.contains("denied"));
    Ok(())
}

/// Baseline: whitespace-only model names are rejected before transport.
#[test]
fn whitespace_model_is_rejected() {
    assert!(
        bench(&Suite {
            model: " \t\n".into(),
            ..Suite::default()
        })
        .is_err()
    );
}
