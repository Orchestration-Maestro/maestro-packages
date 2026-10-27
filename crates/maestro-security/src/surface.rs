//! Attack-surface management: deterministic discovery plus monitored
//! re-discovery, so a target's exposed surface is tracked over time.
//!
//! Discovery runs a port/service probe through the sandbox, then the model
//! structures the result into surface elements. Monitoring re-discovers and
//! diffs against the retained baseline, reporting what was added or removed.

use serde::{Deserialize, Serialize};

use crate::ctx::CapabilityCtx;
use crate::pipeline::Config;
use crate::store::now_secs;

/// Schema tag for the surface document.
pub const SCHEMA: &str = "maestro-security/surface/1";
/// One surface element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Element {
    /// port | service | endpoint | technology | identity.
    pub kind: String,
    /// The concrete detail (e.g. `tcp/22`, `nginx 1.25`).
    pub detail: String,
    /// The probe output that proves it.
    pub evidence: String,
}

/// A discovered attack surface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inventory {
    /// Schema tag.
    pub schema: String,
    /// The assessed target.
    pub target: String,
    /// Unix seconds discovered.
    pub discovered_at: u64,
    /// The surface elements.
    pub elements: Vec<Element>,
}

/// The diff between a baseline and a fresh discovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delta {
    /// Elements present now but not in the baseline.
    pub added: Vec<Element>,
    /// Elements in the baseline but gone now.
    pub removed: Vec<Element>,
    /// Elements unchanged.
    pub unchanged: usize,
}

/// Parameters for a surface action.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Probe {
    /// Target to probe.
    pub target: String,
    /// Action: discover | monitor.
    pub action: String,
    /// Model override.
    pub model: Option<String>,
}

/// The result of a surface action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reading {
    /// Schema tag.
    pub schema: String,
    /// The action performed.
    pub action: String,
    /// The target.
    pub target: String,
    /// The fresh discovery.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface: Option<Inventory>,
    /// The baseline diff, for `monitor`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff: Option<Delta>,
    /// A human-readable message.
    pub message: String,
}

/// Dispatch a surface action.
///
/// # Errors
/// Fails on an unknown action, an empty target, or a probe/model failure.
pub fn dispatch(params: &Probe) -> Result<Reading, String> {
    if params.target.trim().is_empty() {
        return Err("target is required".into());
    }
    let ctx = CapabilityCtx::build(&Config::from_env(), params.model.as_deref())?;
    match params.action.as_str() {
        "discover" => discover(&ctx, &params.target),
        "monitor" => monitor(&ctx, &params.target),
        other => Err(format!("unknown surface action: {other}")),
    }
}

/// Discover the surface once.
fn discover(ctx: &CapabilityCtx, target: &str) -> Result<Reading, String> {
    let doc = probe_and_model(ctx, target)?;
    let count = doc.elements.len();
    save_baseline(&doc)?;
    Ok(Reading {
        schema: SCHEMA.into(),
        action: "discover".into(),
        target: target.into(),
        surface: Some(doc),
        diff: None,
        message: format!("discovered {count} surface elements"),
    })
}

/// Re-discover and diff against the retained baseline.
fn monitor(ctx: &CapabilityCtx, target: &str) -> Result<Reading, String> {
    let fresh = probe_and_model(ctx, target)?;
    let baseline = load_baseline(target)?;
    let diff = diff_surface(baseline.as_ref(), &fresh);
    let added = diff.added.len();
    let removed = diff.removed.len();
    let unchanged = diff.unchanged;
    save_baseline(&fresh)?;
    Ok(Reading {
        schema: SCHEMA.into(),
        action: "monitor".into(),
        target: target.into(),
        surface: Some(fresh),
        diff: Some(diff),
        message: format!("monitor: {added} added, {removed} removed, {unchanged} unchanged"),
    })
}

/// Probe the target, then have the model structure the surface.
fn probe_and_model(ctx: &CapabilityCtx, target: &str) -> Result<Inventory, String> {
    let probe = ctx
        .sandbox
        .exec(&recon_command(target), 120)
        .map_err(|e| format!("surface probe: {e}"))?;
    let user = format!(
        "Target: {target}\n\nProbe output:\n{}\n\nList the exposed attack \
surface as JSON: {{\"elements\":[{{\"kind\":\"port|service|endpoint|technology|\
identity\",\"detail\":\"...\",\"evidence\":\"...\"}}]}}. Use only what the \
probe shows.",
        truncate(&probe.output, 12000)
    );
    let raw = ctx.chat(
        "You are the attack-surface analyst. You list only what the probe \
output proves; nothing is inferred.",
        &user,
        4096,
    )?;
    let elements = parse_elements(&raw);
    Ok(Inventory {
        schema: SCHEMA.into(),
        target: target.into(),
        discovered_at: now_secs(),
        elements,
    })
}

/// The deterministic probe command: a top-ports scan with a fallback.
fn recon_command(target: &str) -> String {
    format!(
        "nmap -Pn -T4 --top-ports 100 {target} 2>&1 | head -120 \
|| echo 'nmap unavailable'"
    )
}

/// Pull the `elements` array out of a model response.
fn parse_elements(raw: &str) -> Vec<Element> {
    let json = crate::pipeline::extract_json(raw);
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) else {
        return Vec::new();
    };
    value
        .pointer("/elements")
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|e| {
                    Some(Element {
                        kind: e.get("kind")?.as_str()?.to_string(),
                        detail: e.get("detail")?.as_str()?.to_string(),
                        evidence: e
                            .get("evidence")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Diff a baseline against a fresh discovery by (kind, detail).
fn diff_surface(baseline: Option<&Inventory>, fresh: &Inventory) -> Delta {
    let mut added = Vec::new();
    let mut unchanged = 0usize;
    for element in &fresh.elements {
        let present = baseline.is_some_and(|b| {
            b.elements
                .iter()
                .any(|e| e.kind == element.kind && e.detail == element.detail)
        });
        if present {
            unchanged += 1;
        } else {
            added.push(element.clone());
        }
    }
    let mut removed = Vec::new();
    if let Some(base) = baseline {
        for element in &base.elements {
            let still = fresh
                .elements
                .iter()
                .any(|e| e.kind == element.kind && e.detail == element.detail);
            if !still {
                removed.push(element.clone());
            }
        }
    }
    Delta {
        added,
        removed,
        unchanged,
    }
}

/// The per-target baseline file path.
fn baseline_path(target: &str) -> std::path::PathBuf {
    let slug: String = target
        .chars()
        .take(40)
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    crate::evidence::base_dir()
        .unwrap_or_default()
        .join("surface")
        .join(format!("{slug}.json"))
}

/// Persist the fresh discovery as the baseline.
///
/// # Errors
/// Fails if the state directory cannot be created or the write fails.
fn save_baseline(doc: &Inventory) -> Result<(), String> {
    let path = baseline_path(&doc.target);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("surface dir: {e}"))?;
    }
    let text = serde_json::to_string_pretty(doc).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("surface write: {e}"))
}

/// Load the retained baseline for a target; missing is `None`.
fn load_baseline(target: &str) -> Result<Option<Inventory>, String> {
    let path = baseline_path(target);
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(None);
    };
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|e| format!("surface baseline: {e}"))
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
    //! Unit tests for surface diffing.

    use super::{Element, Inventory, diff_surface};

    fn doc(elements: Vec<Element>) -> Inventory {
        Inventory {
            schema: "t".into(),
            target: "t".into(),
            discovered_at: 0,
            elements,
        }
    }

    fn el(kind: &str, detail: &str) -> Element {
        Element {
            kind: kind.into(),
            detail: detail.into(),
            evidence: String::new(),
        }
    }

    /// A new element is added; a shared one is unchanged.
    #[test]
    fn diff_detects_added_and_unchanged() {
        let base = doc(vec![el("port", "tcp/22")]);
        let fresh = doc(vec![el("port", "tcp/22"), el("port", "tcp/80")]);
        let diff = diff_surface(Some(&base), &fresh);
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.unchanged, 1);
        assert!(diff.removed.is_empty());
    }

    /// No baseline means everything is added.
    #[test]
    fn diff_without_baseline_is_all_added() {
        let fresh = doc(vec![el("port", "tcp/22")]);
        let diff = diff_surface(None, &fresh);
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.unchanged, 0);
    }
}
