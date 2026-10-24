//! The wasm extension surface: registers the framework's tools with the
//! Maestro host.

use std::fmt::Write;
use std::rc::Rc;

use maestro_extensions_wasm::{
    AgentToolResult, ExtensionAPI, ExtensionFuture, Presence, TextContent, ToolDefinition,
    ToolMetadata, UserBlock,
};
use serde::Deserialize;

use crate::pipeline::{assess, evidence_for, AssessParams};
use crate::report::Assessment;
use crate::tools;

/// Parameters of the evidence tool.
#[derive(Deserialize)]
struct EvidenceParams {
    /// Run id from a finished assessment report.
    run_id: String,
}

/// The `security_assess` tool definition.
fn assess_tool() -> ToolDefinition {
    ToolDefinition {
        metadata: ToolMetadata {
            name: tools::ASSESS.into(),
            label: "Security Assess".into(),
            description: tools::ASSESS_DESCRIPTION.into(),
            parameters: tools::assess_schema().into(),
            prompt_snippet: Presence::Missing,
            prompt_guidelines: Presence::Missing,
            render_shell: Presence::Missing,
            execution_mode: Presence::Missing,
        },
        prepare_arguments: None,
        execute: Rc::new(|_, params, _, _, _| {
            Box::pin(async move {
                let params: AssessParams =
                    serde_json::from_value(params).map_err(|error| error.to_string())?;
                let report: Assessment =
                    assess(&params).map_err(|error| format!("assessment failed: {error}"))?;
                let details =
                    serde_json::to_string(&report).map_err(|error| error.to_string())?;
                Ok(AgentToolResult {
                    content: vec![UserBlock::Text(TextContent::new(
                        human_summary(&report),
                    ))],
                    details: Presence::Present(details),
                    terminate: Presence::Missing,
                })
            })
        }),
    }
}

/// The `security_evidence` tool definition.
fn evidence_tool() -> ToolDefinition {
    ToolDefinition {
        metadata: ToolMetadata {
            name: tools::EVIDENCE.into(),
            label: "Security Evidence".into(),
            description: tools::EVIDENCE_DESCRIPTION.into(),
            parameters: tools::evidence_schema().into(),
            prompt_snippet: Presence::Missing,
            prompt_guidelines: Presence::Missing,
            render_shell: Presence::Missing,
            execution_mode: Presence::Missing,
        },
        prepare_arguments: None,
        execute: Rc::new(|_, params, _, _, _| {
            Box::pin(async move {
                let params: EvidenceParams =
                    serde_json::from_value(params).map_err(|error| error.to_string())?;
                let ledger = evidence_for(&params.run_id)
                    .map_err(|error| format!("evidence lookup failed: {error}"))?;
                Ok(AgentToolResult {
                    content: vec![UserBlock::Text(TextContent::new(format!(
                        "Evidence ledger {} ({} chars). Full JSON in details.",
                        params.run_id,
                        ledger.len()
                    )))],
                    details: Presence::Present(ledger),
                    terminate: Presence::Missing,
                })
            })
        }),
    }
}

/// One-screen human rendering of the finished report.
fn human_summary(report: &Assessment) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "Security run {} ({}): {} (model {})",
        report.run_id, report.agent, report.target, report.model
    );
    let _ = writeln!(out, "Findings: {}", report.findings.len());
    for finding in &report.findings {
        let _ = writeln!(
            out,
            "- [{}] {} ({}, {})",
            finding.severity.to_uppercase(),
            finding.title,
            finding.class,
            finding.evidence_level
        );
    }
    if !report.threats.is_empty() {
        let _ = writeln!(out, "Threats: {}", report.threats.len());
    }
    if !report.unknown.is_empty() {
        let _ = writeln!(out, "Unknown (looked for, not proven): {}", report.unknown.len());
    }
    let _ = writeln!(out, "Refuted hypotheses (coverage): {}", report.refuted.len());
    let _ = writeln!(out, "Summary: {}", report.summary);
    out.push_str("Full report with evidence ids in details.\n");
    out
}

/// Register the security tools with the extension host.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        api.register_tool(assess_tool())?;
        api.register_tool(evidence_tool())?;
        Ok(())
    })
}
