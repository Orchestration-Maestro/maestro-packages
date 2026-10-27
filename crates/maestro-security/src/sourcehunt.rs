//! Source-code vulnerability hunting: deterministic pattern discovery across
//! a source tree through the sandbox, then model judgement of each hit.
//!
//! Discovery runs one bounded `grep` per pattern and collects `file:line`
//! hits. The model then triages the collected hits in a single call, marking
//! each as a real issue or a false positive with a severity and a note.

use serde::{Deserialize, Serialize};

use crate::ctx::CapabilityCtx;
use crate::pipeline::Config;
use crate::runner::shell_quote;

/// Schema tag for the source-hunt result.
pub const SCHEMA: &str = "maestro-security/sourcehunt/1";
/// Default patterns when none are supplied: `(name, grep -E pattern)`.
const DEFAULT_PATTERNS: [(&str, &str); 7] = [
    ("hardcoded-secret", r#"(password|passwd|secret|api[_-]?key|token)\s*[:=]\s*['\"][^'\"]{4,}"#),
    ("sql-injection", r"(query|exec|execute)\w*\s*\([^)]*\+"),
    ("command-injection", r"(system|exec|popen|shell_exec|passthru)\s*\("),
    ("path-traversal", r"(open|fopen|readFile\w*)\s*\([^)]*(user|input|req|param)"),
    ("eval", r"\beval\s*\("),
    ("weak-crypto", r"\b(md5|sha1)\s*\("),
    ("debug-leftover", r"(debug\s*=\s*true|DEBUG_MODE|printenv|var_dump)"),
];
/// Maximum raw hits per pattern.
const MAX_HITS_PER_PATTERN: usize = 40;

/// Parameters for a source hunt.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Query {
    /// Path to the source tree or file.
    pub path: String,
    /// Grep -E patterns to search; empty uses the defaults.
    pub patterns: Vec<String>,
    /// Model override.
    pub model: Option<String>,
}

/// One collected and judged hit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hit {
    /// File path (as reported by grep).
    pub file: String,
    /// Line number.
    pub line: u32,
    /// The pattern name that matched.
    pub pattern: String,
    /// The matched line, trimmed.
    pub text: String,
    /// critical | high | medium | low | info.
    pub severity: String,
    /// The model's judgement in one sentence.
    pub note: String,
}

/// The result of a source hunt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Triage {
    /// Schema tag.
    pub schema: &'static str,
    /// The path hunted.
    pub path: String,
    /// The judged hits.
    pub hits: Vec<Hit>,
    /// One-line summary.
    pub summary: String,
}

/// Run a source hunt: discover hits, then have the model triage them.
///
/// # Errors
/// Fails if the path is empty or the sandbox/model call fails.
pub fn hunt(params: &Query) -> Result<Triage, String> {
    if params.path.trim().is_empty() {
        return Err("path is required".into());
    }
    let ctx = CapabilityCtx::build(&Config::from_env(), params.model.as_deref())?;
    let raw_hits = discover(&ctx, params)?;
    let hits = triage(&ctx, params, &raw_hits)?;
    let summary = format!(
        "Source hunt of {}: {} hits, {} kept",
        params.path,
        raw_hits.len(),
        hits.len()
    );
    Ok(Triage {
        schema: SCHEMA,
        path: params.path.clone(),
        hits,
        summary,
    })
}

/// Run one bounded grep per pattern and collect raw hits.
fn discover(ctx: &CapabilityCtx, params: &Query) -> Result<Vec<Hit>, String> {
    let patterns: Vec<(String, String)> = if params.patterns.is_empty() {
        DEFAULT_PATTERNS
            .iter()
            .map(|(name, pattern)| (name.to_string(), pattern.to_string()))
            .collect()
    } else {
        params
            .patterns
            .iter()
            .map(|p| (p.clone(), p.clone()))
            .collect()
    };
    let mut hits = Vec::new();
    for (name, pattern) in patterns {
        let command = format!(
            "grep -rnIE -- {} {} 2>/dev/null | head -{MAX_HITS_PER_PATTERN}",
            shell_quote(&pattern),
            shell_quote(&params.path)
        );
        let result = ctx
            .sandbox
            .exec(&command, 60)
            .map_err(|e| format!("sourcehunt grep: {e}"))?;
        for line in result.output.lines() {
            if let Some(hit) = parse_hit(line, &name) {
                hits.push(hit);
            }
        }
    }
    Ok(hits)
}

/// Parse one `file:line:text` grep line into a raw hit.
fn parse_hit(line: &str, pattern: &str) -> Option<Hit> {
    let (file, rest) = line.split_once(':')?;
    let (line_no, text) = rest.split_once(':')?;
    let line = line_no.trim().parse::<u32>().ok()?;
    Some(Hit {
        file: file.to_string(),
        line,
        pattern: pattern.to_string(),
        text: text.trim().chars().take(200).collect(),
        severity: "info".into(),
        note: String::new(),
    })
}

/// Have the model triage the raw hits in one call.
fn triage(
    ctx: &CapabilityCtx,
    params: &Query,
    raw: &[Hit],
) -> Result<Vec<Hit>, String> {
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    let listing: String = raw
        .iter()
        .enumerate()
        .map(|(i, h)| format!("[{i}] {}:{} ({}) {}", h.file, h.line, h.pattern, h.text))
        .collect::<Vec<_>>()
        .join("\n");
    let user = format!(
        "Source tree: {}\n\nRaw hits:\n{listing}\n\nFor each hit, decide if it \
is a real security issue. Return JSON: {{\"judgements\":[{{\"index\":0,\
\"keep\":true,\"severity\":\"critical|high|medium|low|info\",\"note\":\"...\"\
}}]}}. Drop false positives (keep:false).",
        params.path
    );
    let raw_json = ctx.chat(
        "You are the source-code security triager. You keep only real issues \
and mark false positives.",
        &user,
        8192,
    )?;
    Ok(apply_judgements(raw, &raw_json))
}

/// Apply the model's judgements to the raw hits.
fn apply_judgements(raw: &[Hit], raw_json: &str) -> Vec<Hit> {
    let json = crate::pipeline::extract_json(raw_json);
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) else {
        // No parseable judgement: return the raw hits untriaged.
        return raw.to_vec();
    };
    let judgements = value
        .pointer("/judgements")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    raw.iter()
        .enumerate()
        .filter_map(|(i, hit)| {
            let j = judgements
                .iter()
                .find(|j| j.get("index").and_then(serde_json::Value::as_u64) == Some(i as u64))?;
            let keep = j.get("keep").and_then(serde_json::Value::as_bool).unwrap_or(true);
            if !keep {
                return None;
            }
            Some(Hit {
                severity: j
                    .get("severity")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("info")
                    .to_string(),
                note: j
                    .get("note")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                ..hit.clone()
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    //! Unit tests for source-hunt helpers.

    use super::{parse_hit, shell_quote};

    /// A well-formed grep line parses into a hit.
    #[test]
    fn parses_grep_line() {
        let hit = parse_hit("src/app.rs:12:let password = \"hunter2\";", "hardcoded-secret")
            .expect("hit");
        assert_eq!(hit.file, "src/app.rs");
        assert_eq!(hit.line, 12);
        assert_eq!(hit.pattern, "hardcoded-secret");
    }

    /// A line with no line number is rejected.
    #[test]
    fn rejects_malformed_line() {
        assert!(parse_hit("no-colon-here", "p").is_none());
    }

    /// Shell quoting escapes embedded single quotes.
    #[test]
    fn quotes_single_quotes() {
        assert_eq!(shell_quote("a'b"), "'a'\\''b'");
    }
}
