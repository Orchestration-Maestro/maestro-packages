//! Baseline benchmark behaviours exercised through the public entry point.

use crate::handler_support as support;

use maestro_security::bench::{Suite, bench};
use serde_json::Value;

/// Expected prompt, distinct answer suffix and grade for each benchmark question.
const QUESTIONS: [(&str, &str, bool); 3] = [
    (
        "Which is more secure: HTTP or HTTPS for transmitting credentials?",
        "transport-answer",
        true,
    ),
    ("What does SQL injection exploit?", "query-answer", false),
    (
        "Name a weak hashing algorithm for passwords.",
        "hash-answer",
        false,
    ),
];

/// Grade one answer and leave malformed or non-boolean grades uncredited.
fn graded(path: &str, body: &Value) -> (u16, String) {
    if path == "/models" {
        return support::models();
    }
    let user = body["messages"][1]["content"].as_str().unwrap_or("");
    let model = body["model"].as_str().unwrap_or("");
    for (prompt, suffix, _) in QUESTIONS {
        let answer = format!("{model}:{suffix}");
        if user == prompt {
            return support::completion(&answer);
        }
        if user.contains(prompt) && user.contains(&answer) {
            return support::completion(match suffix {
                "transport-answer" => "```json\n{\"correct\":true}\n```",
                "query-answer" => "not JSON",
                _ => "{\"correct\":\"true\"}",
            });
        }
    }
    (400, "unexpected benchmark request".into())
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
        assert_eq!(score.answers.len(), QUESTIONS.len());
        for (answer, (prompt, suffix, correct)) in score.answers.iter().zip(QUESTIONS) {
            assert_eq!(answer.prompt, prompt);
            assert_eq!(answer.answer, format!("{model}:{suffix}"));
            assert_eq!(answer.correct, correct);
        }
    }
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
fn whitespace_model_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    if !support::isolated("bench::whitespace_model_is_rejected", denied)? {
        return Ok(());
    }
    let error = bench(&Suite {
        model: " \t\n".into(),
        ..Suite::default()
    })
    .unwrap_err();
    assert_eq!(error, "model is required");
    Ok(())
}
