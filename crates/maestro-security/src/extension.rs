//! The wasm extension surface: registers the framework's tools with the
//! Maestro host.

use std::rc::Rc;

use maestro_extensions_wasm::{
    AgentToolResult, ExtensionAPI, ExtensionFuture, Presence, TextContent, ToolDefinition,
    ToolMetadata, UserBlock,
};
use serde::Deserialize;

use crate::pipeline::{assess, evidence_for, AssessParams};
use crate::report::Report;
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
            parameters: tools::assess_schema(),
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
                let report: Report =
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
            parameters: tools::evidence_schema(),
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
fn human_summary(report: &Report) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Security run {} ({}): {} (model {})\n",
        report.run_id, report.agent, report.target, report.model
    ));
    out.push_str(&format!("Findings: {}\n", report.findings.len()));
    for finding in &report.findings {
        out.push_str(&format!(
            "- [{}] {} ({}, {})\n",
            finding.severity.to_uppercase(),
            finding.title,
            finding.class,
            finding.evidence_level
        ));
    }
    if !report.threats.is_empty() {
        out.push_str(&format!("Threats: {}\n", report.threats.len()));
    }
    if !report.unknown.is_empty() {
        out.push_str(&format!(
            "Unknown (looked for, not proven): {}\n",
            report.unknown.len()
        ));
    }
    out.push_str(&format!(
        "Refuted hypotheses (coverage): {}\n",
        report.refuted.len()
    ));
    out.push_str(&format!("Summary: {}\n", report.summary));
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
