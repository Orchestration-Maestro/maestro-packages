//! Web UI: render a finished assessment as self-contained, Forge-branded
//! HTML pages. Two pages per run: the report (findings, threats, spend) and
//! the evidence ledger (every step, every LLM call). Each page is a single
//! file with inline styles and no external assets, so it can be opened
//! directly, served by any static host, or dropped in as a Maestro artifact.

use std::fmt::Write;

use serde::{Deserialize, Serialize};

use crate::evidence::LedgerDoc;
use crate::report::Assessment;

/// Schema tag for the web report.
pub const SCHEMA: &str = "maestro-security/webui/2";

/// Parameters for a web page.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct WebuiParams {
    /// Run id to render (loads the retained report if present).
    pub run_id: String,
}

/// The rendered web page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebReport {
    /// Schema tag.
    pub schema: &'static str,
    /// The run id.
    pub run_id: String,
    /// report | evidence.
    pub page: String,
    /// The self-contained HTML document.
    pub html: String,
}

/// Render the report page for one assessment.
#[must_use]
pub fn render(assessment: &Assessment) -> WebReport {
    WebReport {
        schema: SCHEMA,
        run_id: assessment.run_id.clone(),
        page: "report".into(),
        html: html_document(assessment),
    }
}

/// Render the evidence page for one retained ledger.
///
/// # Errors
/// Fails if the ledger document is malformed.
pub fn render_evidence(doc: &LedgerDoc) -> Result<WebReport, String> {
    Ok(WebReport {
        schema: SCHEMA,
        run_id: doc.run_id.clone(),
        page: "evidence".into(),
        html: evidence_document(doc),
    })
}

/// The Forge palette and type stack, shared by both pages.
const FORGE_CSS: &str = r"
:root{--forge-black:#0E0B09;--oxide:#2A1F1A;--rust-copper:#B7410E;
--burnished-brass:#D9A066;--molten-ember:#FF6A1A;--vector-steel:#5E7383;
--bone:#F2E8DC;--line:#3a2c22;--radius:8px}
*{box-sizing:border-box}
body{margin:0;background:var(--forge-black);color:var(--bone);
font:16px/1.55 'Barlow','Segoe UI',system-ui,sans-serif}
.wrap{max-width:960px;margin:0 auto;padding:40px 24px 80px}
h1{font:700 36px/1.1 'Barlow Condensed','Arial Narrow',sans-serif;
letter-spacing:.01em;margin:0 0 4px}
h1 .mark{color:var(--molten-ember)}
h2{font:600 13px/1.2 'Barlow Condensed','Arial Narrow',sans-serif;
letter-spacing:.14em;text-transform:uppercase;color:var(--vector-steel);
margin:32px 0 12px}
.meta{color:var(--vector-steel);font-size:13px;margin-bottom:24px;
font-family:'JetBrains Mono',ui-monospace,monospace}
table{width:100%;border-collapse:collapse;background:var(--oxide);
border:1px solid var(--line);border-radius:var(--radius);overflow:hidden}
th,td{text-align:left;padding:10px 14px;border-bottom:1px solid var(--line);
vertical-align:top;font-size:14px}
th{color:var(--vector-steel);font-size:12px;letter-spacing:.06em;
text-transform:uppercase}
tr:last-child td{border-bottom:none}
.sev{font-weight:600}
.critical,.high{color:var(--molten-ember)}.medium{color:var(--burnished-brass)}
.low,.info{color:var(--vector-steel)}
.code{font-family:'JetBrains Mono',ui-monospace,monospace;font-size:12px;
color:var(--vector-steel);word-break:break-all}
.summary{background:var(--oxide);border:1px solid var(--line);
border-left:3px solid var(--rust-copper);border-radius:var(--radius);
padding:16px 18px}
.badge{display:inline-block;padding:2px 8px;border-radius:4px;font-size:11px;
letter-spacing:.06em;text-transform:uppercase;font-weight:600}
.b-confirmed{background:var(--rust-copper);color:var(--bone)}
.b-needs{background:var(--vector-steel);color:var(--forge-black)}
.b-rejected{background:transparent;color:var(--vector-steel);
border:1px solid var(--line)}
.conf{font-family:'JetBrains Mono',ui-monospace,monospace;font-size:12px;
color:var(--burnished-brass)}
ul{margin:8px 0;padding-left:20px;color:var(--vector-steel)}
.empty{color:var(--vector-steel);font-family:'JetBrains Mono',ui-monospace,
monospace;font-size:12px}
.graph{display:flex;gap:16px;flex-wrap:wrap;margin:12px 0}
.groot{align-self:flex-start;border:1px solid var(--molten-ember);
color:var(--molten-ember);border-radius:4px;padding:6px 10px;font-size:13px;
font-family:'JetBrains Mono',ui-monospace,monospace}
.gcol{border-left:2px solid var(--line);padding-left:12px;min-width:180px;
flex:1}
.glabel{font:600 11px/1.2 'Barlow Condensed','Arial Narrow',sans-serif;
letter-spacing:.14em;text-transform:uppercase;color:var(--vector-steel);
margin-bottom:6px}
.gnode{border:1px solid var(--line);border-radius:4px;padding:6px 10px;
font-size:13px;margin:6px 0;background:var(--oxide)}
.gnode.dead{opacity:.55;text-decoration:line-through}
.gnode .gdet{font-family:'JetBrains Mono',ui-monospace,monospace;font-size:11px;
color:var(--vector-steel)}
";

/// Build the report page.
fn html_document(a: &Assessment) -> String {
    let findings = findings_rows(a);
    let refuted = refuted_rows(a);
    let unknown = unknown_list(a);
    let threats = threat_rows(a);
    let spend = spend_line(a);
    format!(
        r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Maestro Security — {target}</title>
<style>{css}</style></head><body><div class="wrap">
<h1><span class="mark">Maestro</span> Security</h1>
<div class="meta">target {target} &middot; agent {agent} &middot; model {model} &middot; run {run_id} {spend}</div>
<div class="summary">{summary}</div>
<h2>Attack graph</h2>
{graph}
<h2>Findings</h2>
{findings}
<h2>Refuted</h2>
{refuted}
<h2>Threat model</h2>
{threats}
<h2>Unknown</h2>
{unknown}
</div></body></html>"#,
        target = esc(&a.target),
        agent = esc(&a.agent),
        model = esc(&a.model),
        run_id = esc(&a.run_id),
        summary = esc(&a.summary),
        graph = attack_graph(a),
        findings = findings,
        refuted = refuted,
        threats = threats,
        unknown = unknown,
        spend = spend,
        css = FORGE_CSS,
    )
}

/// The attack graph section: the target, its findings, threats, dead ends
/// and open questions as a self-contained tree. Only edges present in the
/// data model are drawn.
fn attack_graph(a: &Assessment) -> String {
    let findings = graph_column("findings", a.findings.iter().map(node_finding));
    let threats = graph_column("threats", a.threats.iter().map(node_threat));
    let dead_ends = graph_column("dead ends", a.refuted.iter().map(node_dead_end));
    let open = graph_column(
        "open",
        a.unknown
            .iter()
            .map(|u| format!("<div class='gnode'>{}</div>", esc(u))),
    );
    format!(
        "<div class='graph'><div class='groot'>{target}</div>{findings}{threats}{dead_ends}{open}</div>",
        target = esc(&a.target)
    )
}

/// One finding node: verdict-styled, with its evidence command as detail.
fn node_finding(f: &crate::report::Finding) -> String {
    let cls = match f.verdict {
        crate::report::Verdict::Confirmed | crate::report::Verdict::NeedsValidation => "",
        crate::report::Verdict::Rejected => "dead",
    };
    format!(
        "<div class='gnode {cls}'>{title} <span class='conf'>{conf:.2}</span> <span class='gdet'>{ev}</span></div>",
        title = esc(&f.title),
        conf = f.confidence,
        ev = esc(&f.evidence.command)
    )
}

/// One threat node: the asset, with the impact as detail.
fn node_threat(t: &crate::report::Threat) -> String {
    format!(
        "<div class='gnode'>{asset} <span class='gdet'>{impact}</span></div>",
        asset = esc(&t.asset),
        impact = esc(&t.impact)
    )
}

/// One refuted-hypothesis node: struck through as a dead end.
fn node_dead_end(r: &crate::report::Refuted) -> String {
    format!(
        "<div class='gnode dead'>{hypothesis} <span class='gdet'>{ev}</span></div>",
        hypothesis = esc(&r.hypothesis),
        ev = esc(&r.evidence_id)
    )
}

/// One graph column, or its empty-state line.
fn graph_column<I>(label: &str, nodes: I) -> String
where
    I: IntoIterator<Item = String>,
{
    let nodes: Vec<String> = nodes.into_iter().collect();
    if nodes.is_empty() {
        return format!(
            "<div class='gcol'><div class='glabel'>{label}</div><p class='empty'>none</p></div>"
        );
    }
    format!(
        "<div class='gcol'><div class='glabel'>{label}</div>{}</div>",
        nodes.join("")
    )
}

/// The spend footer line, or an empty string.
fn spend_line(a: &Assessment) -> String {
    let Some(spend) = &a.spend else {
        return String::new();
    };
    format!(
        "&middot; {} calls, {} tokens out, {}s",
        spend.llm_calls, spend.tokens_out, spend.duration_secs
    )
}

/// Build the evidence page from a retained ledger.
fn evidence_document(doc: &LedgerDoc) -> String {
    let entries = entries_rows(doc);
    let calls: usize = doc.entries.iter().filter(|e| e.kind == "llm").count();
    let execs: usize = doc.entries.iter().filter(|e| e.kind == "exec").count();
    format!(
        r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Maestro Security — evidence {run_id}</title>
<style>{css}</style></head><body><div class="wrap">
<h1><span class="mark">Maestro</span> Evidence</h1>
<div class="meta">run {run_id} &middot; target {target} &middot; {calls} llm calls &middot; {execs} executions</div>
<h2>Ledger</h2>
{entries}
</div></body></html>"#,
        run_id = esc(&doc.run_id),
        target = esc(&doc.target),
        entries = entries,
        calls = calls,
        execs = execs,
        css = FORGE_CSS,
    )
}

/// The ledger entries table, or an empty-state row.
fn entries_rows(doc: &LedgerDoc) -> String {
    if doc.entries.is_empty() {
        return "<p class='empty'>no entries</p>".to_string();
    }
    let mut rows = String::new();
    for entry in &doc.entries {
        let note = entry
            .command
            .clone()
            .or_else(|| entry.prompt.clone())
            .unwrap_or_else(|| entry.output.chars().take(120).collect());
        let _ = write!(
            rows,
            "<tr><td class='code'>{}</td><td>{}</td><td>{}</td><td>{}</td><td class='code'>{}</td></tr>",
            esc(&entry.id),
            esc(&entry.phase),
            esc(&entry.kind),
            esc(&note),
            esc(entry.model.as_deref().unwrap_or("—"))
        );
    }
    format!(
        "<table><thead><tr><th>Id</th><th>Phase</th><th>Kind</th><th>Note</th><th>Model</th></tr></thead><tbody>{rows}</tbody></table>"
    )
}

/// The findings table, or an empty-state row.
fn findings_rows(a: &Assessment) -> String {
    if a.findings.is_empty() {
        return "<p class='empty'>no findings</p>".to_string();
    }
    let mut rows = String::new();
    for f in &a.findings {
        let verdict = match f.verdict {
            crate::report::Verdict::Confirmed => "b-confirmed",
            crate::report::Verdict::NeedsValidation => "b-needs",
            crate::report::Verdict::Rejected => "b-rejected",
        };
        let verdict_label = match f.verdict {
            crate::report::Verdict::Confirmed => "confirmed",
            crate::report::Verdict::NeedsValidation => "needs validation",
            crate::report::Verdict::Rejected => "rejected",
        };
        let confidence = format!("{:.2}", f.confidence);
        let _ = write!(
            rows,
            "<tr><td class='sev {}'>{}</td><td><span class='badge {}'>{}</span> <span class='conf'>{}</span></td><td>{}</td><td>{}</td><td class='code'>{}</td></tr>",
            esc(&f.severity),
            esc(&f.severity),
            verdict,
            verdict_label,
            confidence,
            esc(&f.title),
            esc(&f.impact),
            esc(&f.evidence.command)
        );
    }
    format!(
        "<table><thead><tr><th>Sev</th><th>Verdict</th><th>Finding</th><th>Impact</th><th>Evidence</th></tr></thead><tbody>{rows}</tbody></table>"
    )
}

/// The refuted table, or an empty-state row.
fn refuted_rows(a: &Assessment) -> String {
    if a.refuted.is_empty() {
        return "<p class='empty'>none</p>".to_string();
    }
    let mut rows = String::new();
    for r in &a.refuted {
        let _ = write!(
            rows,
            "<tr><td>{}</td><td class='code'>{}</td></tr>",
            esc(&r.hypothesis),
            esc(&r.evidence_id)
        );
    }
    format!(
        "<table><thead><tr><th>Hypothesis</th><th>Evidence</th></tr></thead><tbody>{rows}</tbody></table>"
    )
}

/// The threat-model table, or an empty-state row.
fn threat_rows(a: &Assessment) -> String {
    if a.threats.is_empty() {
        return "<p class='empty'>none</p>".to_string();
    }
    let mut rows = String::new();
    for t in &a.threats {
        let _ = write!(
            rows,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(&t.asset),
            esc(&t.path),
            esc(&t.impact),
            esc(&t.hardening)
        );
    }
    format!(
        "<table><thead><tr><th>Asset</th><th>Path</th><th>Impact</th><th>Hardening</th></tr></thead><tbody>{rows}</tbody></table>"
    )
}

/// The unknowns as a list, or an empty-state line.
fn unknown_list(a: &Assessment) -> String {
    if a.unknown.is_empty() {
        return "<p class='empty'>none</p>".to_string();
    }
    let items: Vec<String> = a
        .unknown
        .iter()
        .map(|u| format!("<li>{}</li>", esc(u)))
        .collect();
    format!("<ul>{}</ul>", items.join(""))
}

/// Escape a string for HTML interpolation.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
#[path = "webui_behavior_tests.rs"]
mod behavior_tests;
