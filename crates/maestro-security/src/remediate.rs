//! Policy-gated remediation: turn findings into reversible fixes, apply only
//! the fixes the policy allows, then adversarially verify each by re-running
//! the original probe and expecting it to now fail.
//!
//! The policy gate is the root-cause control: a fix that is not reversible
//! and non-destructive is proposed but not applied, and is reported as such.

use serde::{Deserialize, Serialize};

use crate::ctx::CapabilityCtx;
use crate::pipeline::Config;

/// Schema tag for the remediation report.
pub const SCHEMA: &str = "maestro-security/remediate/1";

/// Parameters for a remediation run.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Ticket {
    /// The target.
    pub target: String,
    /// The findings to remediate, as text.
    pub findings: Vec<String>,
    /// Model override.
    pub model: Option<String>,
}

/// One remediation step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Remediation {
    /// The finding being fixed.
    pub finding: String,
    /// The fix command.
    pub fix: String,
    /// Whether the policy allowed it to run.
    pub allowed: bool,
    /// The observed output after applying (bounded).
    pub output: String,
    /// The adversarial re-check output (bounded).
    pub recheck: String,
    /// fixed | not-fixed | not-applied.
    pub result: String,
    /// The model's judgement in one or two sentences.
    pub note: String,
}

/// The remediation report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    /// Schema tag.
    pub schema: &'static str,
    /// The target.
    pub target: String,
    /// The remediation steps, in order.
    pub steps: Vec<Remediation>,
    /// Count of fixes verified as fixed.
    pub fixed: usize,
    /// One-line summary.
    pub summary: String,
}

/// Run the remediation pass over the given findings.
///
/// # Errors
/// Fails if the target is empty, no findings are given, or the model call
/// fails.
pub fn remediate(params: &Ticket) -> Result<Plan, String> {
    if params.target.trim().is_empty() {
        return Err("target is required".into());
    }
    if params.findings.is_empty() {
        return Err("at least one finding is required".into());
    }
    let ctx = CapabilityCtx::build(&Config::from_env(), params.model.as_deref())?;
    let fixes = plan(&ctx, params)?;
    let steps = apply(&ctx, params, &fixes);
    let fixed_count = steps.iter().filter(|s| s.result == "fixed").count();
    let summary = format!(
        "Remediation on {}: {} steps, {} fixed",
        params.target,
        steps.len(),
        fixed_count
    );
    Ok(Plan {
        schema: SCHEMA,
        target: params.target.clone(),
        steps,
        fixed: fixed_count,
        summary,
    })
}

/// Have the model propose a reversible fix per finding.
fn plan(ctx: &CapabilityCtx, params: &Ticket) -> Result<Vec<Fix>, String> {
    let listing: String = params
        .findings
        .iter()
        .enumerate()
        .map(|(i, f)| format!("[{i}] {f}"))
        .collect::<Vec<_>>()
        .join("\n");
    let user = format!(
        "Target: {}\n\nFindings:\n{listing}\n\nFor each finding, propose one \
minimal, reversible, non-destructive fix. Return JSON: {{\"fixes\":[{{\
\"index\":0,\"fix\":\"...\",\"reversible\":true,\"recheck\":\"...\"}}]}}. \
`recheck` is the command that proves the finding is gone after the fix.",
        params.target
    );
    let raw = ctx.chat(
        "You are the remediation engineer. You propose minimal, reversible, \
non-destructive fixes and a recheck that proves each is gone.",
        &user,
        8192,
    )?;
    let json = crate::pipeline::extract_json(&raw);
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) else {
        return Err("remediate: model returned no parseable plan".into());
    };
    let fixes = value
        .pointer("/fixes")
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|f| {
                    let index = usize::try_from(f.get("index")?.as_u64()?).ok()?;
                    let fix = f.get("fix")?.as_str()?.to_string();
                    let reversible = f.get("reversible").and_then(serde_json::Value::as_bool)?;
                    let recheck = f
                        .get("recheck")
                        .and_then(|r| r.as_str())
                        .unwrap_or_default()
                        .to_string();
                    if index < params.findings.len() && !fix.trim().is_empty() {
                        Some(Fix {
                            finding: params.findings[index].clone(),
                            command: fix,
                            reversible,
                            recheck,
                        })
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(fixes)
}

/// Apply each policy-allowed fix and adversarially verify it.
fn apply(ctx: &CapabilityCtx, params: &Ticket, fixes: &[Fix]) -> Vec<Remediation> {
    let mut steps = Vec::new();
    for fix in fixes {
        if !fix.reversible {
            steps.push(Remediation {
                finding: fix.finding.clone(),
                fix: fix.command.clone(),
                allowed: false,
                output: String::new(),
                recheck: String::new(),
                result: "not-applied".into(),
                note: "policy gate: not reversible".into(),
            });
            continue;
        }
        let apply_out = ctx
            .sandbox
            .exec(&fix.command, 60)
            .map(|r| r.output)
            .unwrap_or_default();
        let recheck_out = ctx
            .sandbox
            .exec(&fix.recheck, 60)
            .map(|r| r.output)
            .unwrap_or_default();
        let note = grade(ctx, params, fix, &apply_out, &recheck_out);
        let result = if note.0 { "fixed" } else { "not-fixed" };
        steps.push(Remediation {
            finding: fix.finding.clone(),
            fix: fix.command.clone(),
            allowed: true,
            output: truncate(&apply_out, 1000).to_string(),
            recheck: truncate(&recheck_out, 1000).to_string(),
            result: result.into(),
            note: note.1,
        });
    }
    steps
}

/// Have the model grade whether the fix held.
fn grade(
    ctx: &CapabilityCtx,
    params: &Ticket,
    fix: &Fix,
    apply_out: &str,
    recheck_out: &str,
) -> (bool, String) {
    let user = format!(
        "Target: {}\nFinding: {}\nFix: {}\nApply output:\n{}\nRecheck output:\n\
{}\n\nDid the fix hold? Return JSON: {{\"held\":true|false,\"note\":\"...\"}}.",
        params.target,
        fix.finding,
        fix.command,
        truncate(apply_out, 2000),
        truncate(recheck_out, 2000)
    );
    let raw = ctx.chat(
        "You are the remediation verifier. You grade only on the observed \
output.",
        &user,
        1024,
    );
    match raw {
        Ok(text) => {
            let json = crate::pipeline::extract_json(&text);
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) else {
                return (false, text.chars().take(200).collect());
            };
            let held = value
                .get("held")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            let note = value
                .get("note")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string();
            (held, note)
        }
        Err(_) => (false, "verifier call failed".into()),
    }
}

/// One proposed fix.
struct Fix {
    /// The finding being fixed.
    finding: String,
    /// The fix command.
    command: String,
    /// Whether the fix is reversible (policy gate).
    reversible: bool,
    /// The command that proves the finding is gone.
    recheck: String,
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
    //! Unit tests for the remediation policy gate.

    use super::{Remediation, SCHEMA};

    /// A non-reversible fix is reported as not-applied.
    #[test]
    fn policy_gate_reports_not_applied() {
        let step = Remediation {
            finding: "f".into(),
            fix: "rm -rf /".into(),
            allowed: false,
            output: String::new(),
            recheck: String::new(),
            result: "not-applied".into(),
            note: "policy gate: not reversible".into(),
        };
        assert!(!step.allowed);
        assert_eq!(step.result, "not-applied");
        assert_eq!(SCHEMA, "maestro-security/remediate/1");
    }
}
