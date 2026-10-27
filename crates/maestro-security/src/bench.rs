//! Model benchmarking and A/B evaluation: run a fixed set of prompts through
//! one or two models, grade each answer, and compare.
//!
//! The benchmark is a small built-in set of security questions with known
//! answers. Each model's answers are graded by a judge call; A/B mode runs
//! the same prompts on two models and reports the score difference.

use serde::{Deserialize, Serialize};

use crate::ctx::CapabilityCtx;
use crate::pipeline::Config;

/// Schema tag for the benchmark report.
pub const SCHEMA: &str = "maestro-security/bench/1";
/// One benchmark item: prompt plus the expected answer.
struct Item {
    /// The prompt.
    prompt: &'static str,
    /// The expected answer.
    expected: &'static str,
}

/// The built-in benchmark set.
const ITEMS: &[Item] = &[
    Item {
        prompt: "Which is more secure: HTTP or HTTPS for transmitting credentials?",
        expected: "HTTPS",
    },
    Item {
        prompt: "What does SQL injection exploit?",
        expected: "unvalidated input in queries",
    },
    Item {
        prompt: "Name a weak hashing algorithm for passwords.",
        expected: "md5 or sha1",
    },
];

/// Parameters for a benchmark.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Suite {
    /// The model to benchmark.
    pub model: String,
    /// A second model for A/B comparison, if any.
    pub compare: Option<String>,
    /// Model override for the judge (defaults to the benchmarked model).
    pub judge: Option<String>,
}

/// One benchmark result for a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Score {
    /// The model.
    pub model: String,
    /// Count of correct answers.
    pub correct: usize,
    /// Total items.
    pub total: usize,
    /// The per-item answers.
    pub answers: Vec<Answer>,
}

/// One item's answer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Answer {
    /// The prompt.
    pub prompt: String,
    /// The model's answer.
    pub answer: String,
    /// Whether the judge marked it correct.
    pub correct: bool,
}

/// The benchmark report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scorecard {
    /// Schema tag.
    pub schema: &'static str,
    /// The primary model's score.
    pub primary: Score,
    /// The comparison model's score, in A/B mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compare: Option<Score>,
    /// One-line summary.
    pub summary: String,
}

/// Run the benchmark on one model, and optionally a second for A/B.
///
/// # Errors
/// Fails if the model is empty or a model call fails.
pub fn bench(params: &Suite) -> Result<Scorecard, String> {
    if params.model.trim().is_empty() {
        return Err("model is required".into());
    }
    let config = Config::from_env();
    let ctx = CapabilityCtx::build(&config, Some(&params.model))?;
    let primary = score(&ctx, &params.model)?;
    let compare = match &params.compare {
        Some(other) => {
            let other_ctx = CapabilityCtx::build(&config, Some(other))?;
            Some(score(&other_ctx, other)?)
        }
        None => None,
    };
    let summary = match &compare {
        Some(c) => format!(
            "A/B: {} {}/{} vs {} {}/{}",
            params.model, primary.correct, primary.total, c.model, c.correct, c.total
        ),
        None => format!(
            "{}: {}/{} correct",
            params.model, primary.correct, primary.total
        ),
    };
    Ok(Scorecard {
        schema: SCHEMA,
        primary,
        compare,
        summary,
    })
}

/// Run every benchmark item through one model and grade the answers.
fn score(ctx: &CapabilityCtx, model: &str) -> Result<Score, String> {
    let mut answers = Vec::new();
    for item in ITEMS {
        let reply = ctx
            .client
            .chat(model, "Answer concisely.", item.prompt, 512)?;
        let answer = reply.content;
        let correct = judge(ctx, item, &answer)?;
        answers.push(Answer {
            prompt: item.prompt.to_string(),
            answer,
            correct,
        });
    }
    let correct = answers.iter().filter(|a| a.correct).count();
    Ok(Score {
        model: model.to_string(),
        correct,
        total: ITEMS.len(),
        answers,
    })
}

/// Have the judge grade one answer against the expected answer.
fn judge(ctx: &CapabilityCtx, item: &Item, answer: &str) -> Result<bool, String> {
    let user = format!(
        "Question: {}\nExpected: {}\nAnswer: {}\n\nIs the answer correct? \
Return JSON: {{\"correct\":true|false}}.",
        item.prompt, item.expected, answer
    );
    let reply = ctx
        .client
        .chat(&ctx.model, "You are the grader.", &user, 256)?;
    let raw = reply.content;
    let json = crate::pipeline::extract_json(&raw);
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) else {
        return Ok(false);
    };
    Ok(value
        .get("correct")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false))
}

#[cfg(test)]
mod tests {
    //! Unit tests for the benchmark.

    use super::{Suite, bench};

    /// An empty model is rejected before any call.
    #[test]
    fn empty_model_is_rejected() {
        let err = bench(&Suite::default()).unwrap_err();
        assert!(err.contains("model"));
    }
}
