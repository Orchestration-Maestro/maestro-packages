//! The wasm extension surface: registers the framework's tools and trigger
//! commands with the Maestro host.

use std::fmt::Write;
use std::rc::Rc;

use maestro_extensions_wasm::{
    AgentToolResult, CallbackEmitter, CommandOptions, EventBus, EventListener,
    ExtensionAPI, ExtensionCommandContext, ExtensionFuture, NotificationType, Presence,
    TextContent, ToolDefinition, ToolMetadata, UserBlock,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::autonomy;
use crate::autonomy::run::{EventSink, SessionEntry};
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

/// The `security_workspace` tool definition.
fn workspace_tool() -> ToolDefinition {
    ToolDefinition {
        metadata: ToolMetadata {
            name: tools::WORKSPACE.into(),
            label: "Security Workspace".into(),
            description: tools::WORKSPACE_DESCRIPTION.into(),
            parameters: tools::workspace_schema().into(),
            prompt_snippet: Presence::Missing,
            prompt_guidelines: Presence::Missing,
            render_shell: Presence::Missing,
            execution_mode: Presence::Missing,
        },
        prepare_arguments: None,
        execute: Rc::new(|_, params, _, _, _| {
            Box::pin(async move {
                let text = tools::run_workspace(&params)
                    .map_err(|error| format!("workspace failed: {error}"))?;
                Ok(AgentToolResult {
                    content: vec![UserBlock::Text(TextContent::new(text))],
                    details: Presence::Missing,
                    terminate: Presence::Missing,
                })
            })
        }),
    }
}

/// The `security_doctor` tool definition.
fn doctor_tool() -> ToolDefinition {
    ToolDefinition {
        metadata: ToolMetadata {
            name: tools::DOCTOR.into(),
            label: "Security Doctor".into(),
            description: tools::DOCTOR_DESCRIPTION.into(),
            parameters: json!("{}").into(),
            prompt_snippet: Presence::Missing,
            prompt_guidelines: Presence::Missing,
            render_shell: Presence::Missing,
            execution_mode: Presence::Missing,
        },
        prepare_arguments: None,
        execute: Rc::new(|_, _, _, _, _| {
            Box::pin(async move {
                let text =
                    tools::run_doctor().map_err(|error| format!("doctor failed: {error}"))?;
                Ok(AgentToolResult {
                    content: vec![UserBlock::Text(TextContent::new(text))],
                    details: Presence::Missing,
                    terminate: Presence::Missing,
                })
            })
        }),
    }
}

/// The `security_route` tool definition.
fn route_tool() -> ToolDefinition {
    ToolDefinition {
        metadata: ToolMetadata {
            name: tools::ROUTE.into(),
            label: "Security Route".into(),
            description: tools::ROUTE_DESCRIPTION.into(),
            parameters: tools::route_schema().into(),
            prompt_snippet: Presence::Missing,
            prompt_guidelines: Presence::Missing,
            render_shell: Presence::Missing,
            execution_mode: Presence::Missing,
        },
        prepare_arguments: None,
        execute: Rc::new(|_, params, _, _, _| {
            Box::pin(async move {
                let text =
                    tools::run_route(&params).map_err(|error| format!("route failed: {error}"))?;
                Ok(AgentToolResult {
                    content: vec![UserBlock::Text(TextContent::new(text))],
                    details: Presence::Missing,
                    terminate: Presence::Missing,
                })
            })
        }),
    }
}

/// The `security_report_page` tool definition.
fn report_page_tool() -> ToolDefinition {
    ToolDefinition {
        metadata: ToolMetadata {
            name: tools::REPORT_PAGE.into(),
            label: "Security Report Page".into(),
            description: tools::REPORT_PAGE_DESCRIPTION.into(),
            parameters: tools::page_schema().into(),
            prompt_snippet: Presence::Missing,
            prompt_guidelines: Presence::Missing,
            render_shell: Presence::Missing,
            execution_mode: Presence::Missing,
        },
        prepare_arguments: None,
        execute: Rc::new(|_, params, _, _, _| {
            Box::pin(async move {
                let html = tools::run_report_page(&params)
                    .map_err(|error| format!("report page failed: {error}"))?;
                Ok(AgentToolResult {
                    content: vec![UserBlock::Text(TextContent::new(html))],
                    details: Presence::Missing,
                    terminate: Presence::Missing,
                })
            })
        }),
    }
}

/// The `security_evidence_page` tool definition.
fn evidence_page_tool() -> ToolDefinition {
    ToolDefinition {
        metadata: ToolMetadata {
            name: tools::EVIDENCE_PAGE.into(),
            label: "Security Evidence Page".into(),
            description: tools::EVIDENCE_PAGE_DESCRIPTION.into(),
            parameters: tools::page_schema().into(),
            prompt_snippet: Presence::Missing,
            prompt_guidelines: Presence::Missing,
            render_shell: Presence::Missing,
            execution_mode: Presence::Missing,
        },
        prepare_arguments: None,
        execute: Rc::new(|_, params, _, _, _| {
            Box::pin(async move {
                let html = tools::run_evidence_page(&params)
                    .map_err(|error| format!("evidence page failed: {error}"))?;
                Ok(AgentToolResult {
                    content: vec![UserBlock::Text(TextContent::new(html))],
                    details: Presence::Missing,
                    terminate: Presence::Missing,
                })
            })
        }),
    }
}

/// The `security_validate` tool definition.
fn validate_tool() -> ToolDefinition {
    ToolDefinition {
        metadata: ToolMetadata {
            name: tools::VALIDATE.into(),
            label: "Security Validate".into(),
            description: tools::VALIDATE_DESCRIPTION.into(),
            parameters: tools::page_schema().into(),
            prompt_snippet: Presence::Missing,
            prompt_guidelines: Presence::Missing,
            render_shell: Presence::Missing,
            execution_mode: Presence::Missing,
        },
        prepare_arguments: None,
        execute: Rc::new(|_, params, _, _, _| {
            Box::pin(async move {
                let text =
                    tools::run_validate(&params).map_err(|error| format!("validate failed: {error}"))?;
                Ok(AgentToolResult {
                    content: vec![UserBlock::Text(TextContent::new(text))],
                    details: Presence::Missing,
                    terminate: Presence::Missing,
                })
            })
        }),
    }
}

/// The `security_disclose` tool definition.
fn disclose_tool() -> ToolDefinition {
    ToolDefinition {
        metadata: ToolMetadata {
            name: tools::DISCLOSE.into(),
            label: "Security Disclose".into(),
            description: tools::DISCLOSE_DESCRIPTION.into(),
            parameters: tools::disclose_schema().into(),
            prompt_snippet: Presence::Missing,
            prompt_guidelines: Presence::Missing,
            render_shell: Presence::Missing,
            execution_mode: Presence::Missing,
        },
        prepare_arguments: None,
        execute: Rc::new(|_, params, _, _, _| {
            Box::pin(async move {
                let text =
                    tools::run_disclose(&params).map_err(|error| format!("disclose failed: {error}"))?;
                Ok(AgentToolResult {
                    content: vec![UserBlock::Text(TextContent::new(text))],
                    details: Presence::Missing,
                    terminate: Presence::Missing,
                })
            })
        }),
    }
}

/// The session entry type that opens a run's live thread.
const RUN_ENTRY: &str = "security-run";

/// Emit the lifecycle's events on the host bus.
struct BusSink(EventBus);

impl EventSink for BusSink {
    fn emit<'a>(&'a mut self, channel: &'a str, payload: Value) -> ExtensionFuture<'a, ()> {
        self.0.emit(channel, payload)
    }
}

/// Append the run's live entries through the host API.
struct ApiEntry(ExtensionAPI);

impl SessionEntry for ApiEntry {
    fn append(&mut self, body: &Value) -> Result<(), String> {
        self.0.append_entry(RUN_ENTRY, Some(body.clone()))
    }
}

/// The synchronous trigger prefix: hand the lifecycle to an independently
/// owned tail, so the command answers at once.
fn trigger_listener(api: ExtensionAPI) -> EventListener {
    Rc::new(move |payload: Value, _emitter: CallbackEmitter<'_>| {
        let api = api.clone();
        Ok(Some(Box::pin(async move {
            let mut sink = BusSink(api.events());
            let mut entry = ApiEntry(api);
            autonomy::run_trigger(&mut sink, &mut entry, &payload).await
        })))
    })
}

/// Register the trigger commands and their listener with the host.
fn register_autonomy(api: &ExtensionAPI) -> Result<(), String> {
    api.events()
        .on(autonomy::TRIGGER_CHANNEL, trigger_listener(api.clone()))
        .map_err(|error| format!("trigger listener: {error}"))?;
    for command in autonomy::COMMANDS {
        let bus = api.events();
        api.register_command(
            command.name,
            CommandOptions {
                description: Some(command.description.to_string()),
                handler: Rc::new(move |args, ctx| {
                    let bus = bus.clone();
                    Box::pin(async move {
                        trigger_command(command.name, &bus, &ctx, &args).await;
                        Ok(())
                    })
                }),
                get_argument_completions: None,
            },
        )
        .map_err(|error| format!("register /{}: {error}", command.name))?;
    }
    Ok(())
}

/// Dispatch one trigger command to its single-purpose handler.
async fn trigger_command(
    name: &str,
    bus: &EventBus,
    ctx: &ExtensionCommandContext,
    args: &str,
) {
    match name {
        autonomy::ASSESS_COMMAND => assess_command(bus, ctx, args).await,
        autonomy::STOP_COMMAND => stop_command(ctx, args),
        autonomy::STEER_COMMAND => steer_command(ctx, args),
        _ => {}
    }
}

/// `/assess <target>`: record the run, emit the trigger, answer with the id.
async fn assess_command(bus: &EventBus, ctx: &ExtensionCommandContext, args: &str) {
    let target = match autonomy::parse_target(args) {
        Ok(target) => target,
        Err(error) => return notify(ctx, &error, true),
    };
    let run_id = match autonomy::start_run(&target) {
        Ok(run_id) => run_id,
        Err(error) => return notify(ctx, &format!("could not record the run: {error}"), true),
    };
    let failed = bus
        .emit(
            autonomy::TRIGGER_CHANNEL,
            autonomy::trigger_payload(&run_id, &target),
        )
        .await;
    if let Err(error) = failed {
        let _ = autonomy::finish_run(&run_id, Err(error.as_str()));
        return notify(ctx, &format!("could not start the run: {error}"), true);
    }
    notify(
        ctx,
        &format!("Assessment {run_id} started. /security-stop {run_id} stops it."),
        false,
    );
}

/// `/security-stop <run_id>`: acknowledge; the abort lands with the run task.
fn stop_command(ctx: &ExtensionCommandContext, args: &str) {
    match autonomy::parse_run_id(args) {
        Ok(run_id) => notify(
            ctx,
            &format!(
                "Stop noted for {run_id}; the abort control lands in T3, so the run keeps going for now."
            ),
            false,
        ),
        Err(error) => notify(ctx, &error, true),
    }
}

/// `/security-steer <run_id> <message>`: acknowledge; steering lands with the
/// run task.
fn steer_command(ctx: &ExtensionCommandContext, args: &str) {
    match autonomy::parse_steer(args) {
        Ok((run_id, message)) => notify(
            ctx,
            &format!(
                "Steer noted for {run_id}: {message}; the steering queue lands in T3."
            ),
            false,
        ),
        Err(error) => notify(ctx, &error, true),
    }
}

/// Show one notification; failures use the error severity.
fn notify(ctx: &ExtensionCommandContext, message: &str, failure: bool) {
    let kind = if failure {
        NotificationType::Error
    } else {
        NotificationType::Info
    };
    if let Ok(ui) = ctx.ui() {
        let _ = ui.notify(message, Some(kind));
    }
}

/// Register the security tools with the extension host.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        api.register_tool(assess_tool())?;
        api.register_tool(evidence_tool())?;
        api.register_tool(workspace_tool())?;
        api.register_tool(doctor_tool())?;
        api.register_tool(route_tool())?;
        api.register_tool(report_page_tool())?;
        api.register_tool(evidence_page_tool())?;
        api.register_tool(validate_tool())?;
        api.register_tool(disclose_tool())?;
        register_autonomy(&api)?;
        Ok(())
    })
}

#[cfg(test)]
#[path = "extension_tests.rs"]
mod tests;
