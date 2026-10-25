//! Reverse engineering: deterministic artifact inspection through the sandbox
//! plus model interpretation, for binaries, documents and archives.
//!
//! The deterministic pass collects what the artifact is (file type), its
//! notable strings, its exported symbols and its sections. The model then
//! interprets the result: what the artifact is, what it does, and the
//! security-relevant observations, with gaps named as unknowns.

use serde::{Deserialize, Serialize};

use crate::ctx::CapabilityCtx;
use crate::pipeline::Config;

/// Schema tag for the reverse-engineering report.
pub const SCHEMA: &str = "maestro-security/reverse/1";

/// Parameters for a reverse-engineering run.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Artifact {
    /// Path to the artifact (binary, document, archive).
    pub path: String,
    /// Model override.
    pub model: Option<String>,
}

/// The reverse-engineering report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Analysis {
    /// Schema tag.
    pub schema: &'static str,
    /// The artifact path.
    pub path: String,
    /// The `file` type line.
    pub file_type: String,
    /// Notable strings, deduplicated and capped.
    pub strings: Vec<String>,
    /// Exported/defined symbols, capped.
    pub symbols: Vec<String>,
    /// Section names, when available.
    pub sections: Vec<String>,
    /// The model that interpreted the artifact.
    pub model: String,
    /// The model's interpretation.
    pub analysis: String,
    /// Looked for but not proven.
    pub unknown: Vec<String>,
}

/// Run a reverse-engineering pass on an artifact.
///
/// # Errors
/// Fails if the path is empty or the sandbox/model call fails.
pub fn reverse(params: &Artifact) -> Result<Analysis, String> {
    if params.path.trim().is_empty() {
        return Err("path is required".into());
    }
    let ctx = CapabilityCtx::build(&Config::from_env(), params.model.as_deref())?;
    let facts = Facts {
        file_type: run(&ctx, &format!("file {}", sh(&params.path))),
        strings: run(&ctx, &format!("strings {} 2>/dev/null | head -200", sh(&params.path))),
        symbols: run(
            &ctx,
            &format!(
                "nm {} 2>/dev/null | head -100 || objdump -t {} 2>/dev/null | head -100",
                sh(&params.path),
                sh(&params.path)
            ),
        ),
        sections: run(
            &ctx,
            &format!(
                "objdump -h {} 2>/dev/null | awk 'NR>2 {{print $2}}' | head -40",
                sh(&params.path)
            ),
        ),
    };
    let (analysis, unknown) = interpret(&ctx, &params.path, &facts)?;
    Ok(Analysis {
        schema: SCHEMA,
        path: params.path.clone(),
        file_type: facts.file_type.lines().next().unwrap_or_default().to_string(),
        strings: dedup_lines(&facts.strings, 120),
        symbols: dedup_lines(&facts.symbols, 80),
        sections: dedup_lines(&facts.sections, 40),
        model: ctx.model,
        analysis,
        unknown,
    })
}

/// Run one sandbox command, returning empty output on failure.
fn run(ctx: &CapabilityCtx, command: &str) -> String {
    ctx.sandbox
        .exec(command, 60)
        .map(|r| r.output)
        .unwrap_or_default()
}

/// Raw artifact facts collected in the sandbox.
#[derive(Debug, Clone)]
struct Facts {
    /// The `file` output.
    file_type: String,
    /// The `strings` output.
    strings: String,
    /// The `nm` / `objdump -t` output.
    symbols: String,
    /// The `objdump -h` output.
    sections: String,
}

/// Have the model interpret the collected artifact facts.
fn interpret(ctx: &CapabilityCtx, path: &str, facts: &Facts) -> Result<(String, Vec<String>), String> {
    let user = format!(
        "Artifact: {path}\n\nfile: {}\n\nNotable strings:\n{}\n\nSymbols:\
\n{}\n\nSections:\n{}\n\nInterpret this artifact. Return JSON: \
{{\"analysis\":\"...\",\"unknown\":[\"...\"]}}. Name what it is, what it does, \
and the security-relevant observations. List gaps as unknowns.",
        facts.file_type,
        truncate(&facts.strings, 8000),
        truncate(&facts.symbols, 4000),
        truncate(&facts.sections, 2000)
    );
    let raw = ctx.chat(
        "You are the reverse engineer. You state only what the artifact facts \
support and name every gap.",
        &user,
        8192,
    )?;
    let json = crate::pipeline::extract_json(&raw);
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) else {
        return Ok((raw, Vec::new()));
    };
    let analysis = value
        .get("analysis")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let unknown = value
        .pointer("/unknown")
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|u| u.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    Ok((analysis, unknown))
}

/// Single-quote a path for safe shell interpolation.
fn sh(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Split into non-empty lines, deduplicated, capped at `max`.
fn dedup_lines(text: &str, max: usize) -> Vec<String> {
    let mut out: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    out.dedup();
    out.truncate(max);
    out
}

/// Cap a string for prompts.
fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

#[cfg(test)]
mod tests {
    //! Unit tests for reverse-engineering helpers.

    use super::dedup_lines;

    /// Dedup removes repeats and caps the length.
    #[test]
    fn dedup_caps_and_dedupes() {
        let out = dedup_lines("a\nb\na\n\nb\nc", 2);
        assert_eq!(out, vec!["a", "b"]);
    }
}
