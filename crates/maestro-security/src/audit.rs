//! Structured security audit: a checklist of named validators run through the
//! sandbox, each judged pass/fail/warn, then model-summarized into an audit
//! report.
//!
//! Validators are `(name, command, pass_pattern)` triples. A validator passes
//! when its command's output matches the pass pattern (a regex), warns when
//! the command runs but the pattern is absent, and fails when the command
//! errors. The model then summarizes the overall posture.

use serde::{Deserialize, Serialize};

use crate::ctx::CapabilityCtx;
use crate::pipeline::Config;

/// Schema tag for the audit report.
pub const SCHEMA: &str = "maestro-security/audit/1";
/// A named validator.
struct Validator {
    /// Human name of the check.
    name: &'static str,
    /// The command to run.
    command: &'static str,
    /// Regex that indicates the check passes.
    pass_pattern: &'static str,
}

/// The default validator checklist.
const DEFAULT_VALIDATORS: &[Validator] = &[
    Validator {
        name: "no-world-writable-files",
        command: "find / -xdev -type f -perm -0002 2>/dev/null | head -5",
        pass_pattern: r"^$",
    },
    Validator {
        name: "no-empty-passwords",
        command: "awk -F: '($2 == \"\") {print $1}' /etc/shadow 2>/dev/null | head -5",
        pass_pattern: r"^$",
    },
    Validator {
        name: "ssh-no-root-login",
        command: "grep -Ei '^PermitRootLogin' /etc/ssh/sshd_config 2>/dev/null | grep -vi yes",
        pass_pattern: r"(yes|no|PermitRootLogin)",
    },
    Validator {
        name: "no-debug-symbols-in-binaries",
        command: "file /bin/ls 2>/dev/null | grep -c 'not stripped'",
        pass_pattern: r"0",
    },
    Validator {
        name: "tmp-is-nosuid",
        command: "mount 2>/dev/null | grep ' /tmp ' | grep -c nosuid",
        pass_pattern: r"1",
    },
];

/// Parameters for an audit.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Scope {
    /// Target description (host or scope label).
    pub target: String,
    /// Model override.
    pub model: Option<String>,
}

/// One validator result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Check {
    /// The validator name.
    pub name: String,
    /// pass | fail | warn.
    pub status: String,
    /// The command output (bounded).
    pub evidence: String,
    /// One-line note.
    pub note: String,
}

/// The audit report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    /// Schema tag.
    pub schema: &'static str,
    /// The audited target.
    pub target: String,
    /// The per-validator results.
    pub checks: Vec<Check>,
    /// Count of passes.
    pub passed: usize,
    /// Count of failures.
    pub failed: usize,
    /// Count of warnings.
    pub warned: usize,
    /// The model's overall posture summary.
    pub summary: String,
}

/// Run the audit: execute every validator, then summarize.
///
/// # Errors
/// Fails if the target is empty or the model call fails.
pub fn audit(params: &Scope) -> Result<Report, String> {
    if params.target.trim().is_empty() {
        return Err("target is required".into());
    }
    let ctx = CapabilityCtx::build(&Config::from_env(), params.model.as_deref())?;
    let checks = run_validators(&ctx);
    let counts = tally(&checks);
    let (passed, failed, warned) = counts;
    let summary = summarize(&ctx, &params.target, &checks, counts)?;
    Ok(Report {
        schema: SCHEMA,
        target: params.target.clone(),
        checks,
        passed,
        failed,
        warned,
        summary,
    })
}

/// Run every default validator and judge its result.
fn run_validators(ctx: &CapabilityCtx) -> Vec<Check> {
    DEFAULT_VALIDATORS
        .iter()
        .map(|v| judge_validator(ctx, v))
        .collect()
}

/// Run one validator and judge pass/fail/warn.
fn judge_validator(ctx: &CapabilityCtx, v: &Validator) -> Check {
    let result = ctx.sandbox.exec(v.command, 60);
    match result {
        Ok(exec) => {
            let matched = regex_match(v.pass_pattern, &exec.output);
            let status = if matched { "pass" } else { "warn" };
            Check {
                name: v.name.into(),
                status: status.into(),
                evidence: truncate(&exec.output, 500).to_string(),
                note: format!("exit {}", exec.exit_code),
            }
        }
        Err(error) => Check {
            name: v.name.into(),
            status: "fail".into(),
            evidence: truncate(&error, 500).to_string(),
            note: "command error".into(),
        },
    }
}

/// Count pass/fail/warn.
fn tally(checks: &[Check]) -> (usize, usize, usize) {
    let passed = checks.iter().filter(|c| c.status == "pass").count();
    let failed = checks.iter().filter(|c| c.status == "fail").count();
    let warned = checks.iter().filter(|c| c.status == "warn").count();
    (passed, failed, warned)
}

/// Have the model summarize the overall posture.
fn summarize(
    ctx: &CapabilityCtx,
    target: &str,
    checks: &[Check],
    counts: (usize, usize, usize),
) -> Result<String, String> {
    let (passed, failed, warned) = counts;
    let listing: String = checks
        .iter()
        .map(|c| format!("- {} [{}]: {}", c.name, c.status, c.note))
        .collect::<Vec<_>>()
        .join("\n");
    let user = format!(
        "Audit of {target}: {passed} passed, {failed} failed, {warned} warned.\n\
Checks:\n{listing}\n\nSummarize the overall security posture in three \
sentences and name the top two risks."
    );
    ctx.chat(
        "You are the security auditor. You summarize only what the checks show.",
        &user,
        2048,
    )
}

/// A small regex matcher: supports a literal `^$` (empty) and a plain
/// substring/number otherwise, avoiding a regex crate dependency.
fn regex_match(pattern: &str, text: &str) -> bool {
    if pattern == r"^$" {
        return text.trim().is_empty();
    }
    text.lines().any(|line| line.trim() == pattern) || text.contains(pattern)
}

/// Cap a string for reports.
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
    //! Unit tests for audit helpers.

    use super::regex_match;

    /// An empty-output pattern passes only on empty text.
    #[test]
    fn empty_pattern_needs_empty_output() {
        assert!(regex_match(r"^$", "  \n"));
        assert!(!regex_match(r"^$", "some output"));
    }

    /// A value pattern matches when present in the output.
    #[test]
    fn value_pattern_matches_presence() {
        assert!(regex_match("0", "0\n"));
        assert!(!regex_match("1", "0\n"));
    }
}
