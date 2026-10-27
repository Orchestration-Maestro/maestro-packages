//! The MCP stdio surface: the shared tool runners behind the rmcp server.
//!
//! One handler serves the full security tool registry over the Model Context
//! Protocol on stdio. The extension surface exposes the same runners as
//! Maestro tools, so both surfaces stay in lockstep.

use std::future::Future;

use rmcp::ErrorData as McpError;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    JsonObject, ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
};
use rmcp::service::{RequestContext, RoleServer, serve_server};
use rmcp::transport::stdio;

use crate::tools;

/// The MCP server: one handler for the full security tool registry.
pub struct SecurityServer;

impl SecurityServer {
    /// The tool registry, built from the shared names and schemas.
    fn registry() -> Vec<Tool> {
        [
            (
                tools::ASSESS,
                tools::ASSESS_DESCRIPTION,
                tools::assess_schema(),
            ),
            (
                tools::EVIDENCE,
                tools::EVIDENCE_DESCRIPTION,
                tools::evidence_schema(),
            ),
            (
                tools::WORKSPACE,
                tools::WORKSPACE_DESCRIPTION,
                tools::workspace_schema(),
            ),
            (
                tools::DOCTOR,
                tools::DOCTOR_DESCRIPTION,
                serde_json::json!({}),
            ),
            (
                tools::ROUTE,
                tools::ROUTE_DESCRIPTION,
                tools::route_schema(),
            ),
            (
                tools::REPORT_PAGE,
                tools::REPORT_PAGE_DESCRIPTION,
                tools::page_schema(),
            ),
            (
                tools::EVIDENCE_PAGE,
                tools::EVIDENCE_PAGE_DESCRIPTION,
                tools::page_schema(),
            ),
            (
                tools::VALIDATE,
                tools::VALIDATE_DESCRIPTION,
                tools::page_schema(),
            ),
            (
                tools::DISCLOSE,
                tools::DISCLOSE_DESCRIPTION,
                tools::disclose_schema(),
            ),
        ]
        .into_iter()
        .map(|(name, description, schema)| Tool::new(name, description, schema_object(&schema)))
        .collect()
    }

    /// Dispatch one call to the shared runner; tool errors are results, not
    /// protocol errors.
    fn call(name: &str, arguments: &serde_json::Value) -> Result<String, String> {
        match name {
            tools::ASSESS => tools::run_assess(arguments),
            tools::EVIDENCE => tools::run_evidence(arguments),
            tools::WORKSPACE => tools::run_workspace(arguments),
            tools::DOCTOR => tools::run_doctor(),
            tools::ROUTE => tools::run_route(arguments),
            tools::REPORT_PAGE => tools::run_report_page(arguments),
            tools::EVIDENCE_PAGE => tools::run_evidence_page(arguments),
            tools::VALIDATE => tools::run_validate(arguments),
            tools::DISCLOSE => tools::run_disclose(arguments),
            other => Err(format!("unknown tool: {other}")),
        }
    }
}

/// The object form of a schema value; schemas are always objects.
fn schema_object(schema: &serde_json::Value) -> JsonObject {
    schema.as_object().cloned().unwrap_or_default()
}

impl ServerHandler for SecurityServer {
    /// The server identity and capabilities.
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new("maestro-security", env!("CARGO_PKG_VERSION")),
        )
    }

    /// The full tool registry.
    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, McpError>> {
        std::future::ready(Ok(ListToolsResult::with_all_items(Self::registry())))
    }

    /// One tool call; a failed run is an error result the caller can read.
    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResponse, McpError>> {
        let arguments = request
            .arguments
            .map(serde_json::Value::Object)
            .unwrap_or_default();
        let outcome = Self::call(&request.name, &arguments);
        let result = match outcome {
            Ok(text) => CallToolResult::success(vec![ContentBlock::text(text)]),
            Err(message) => CallToolResult::error(vec![ContentBlock::text(message)]),
        };
        std::future::ready(Ok(result.into()))
    }
}

/// Serve the MCP server on stdio until the client disconnects.
///
/// # Errors
/// Fails if the peer never completes the initialize handshake or the
/// service task panics.
pub async fn serve() -> Result<(), String> {
    let running = serve_server(SecurityServer, stdio())
        .await
        .map_err(|error| format!("mcp initialize: {error}"))?;
    running
        .waiting()
        .await
        .map(|_| ())
        .map_err(|error| format!("mcp serve: {error}"))
}

#[cfg(test)]
mod tests {
    //! Unit tests for the MCP surface.

    use super::SecurityServer;

    /// The registry exposes the full nine-tool set, assess first.
    #[test]
    fn registry_has_the_full_set() {
        let registry = SecurityServer::registry();
        assert_eq!(registry.len(), 9);
        assert_eq!(registry[0].name, "security_assess");
        assert_eq!(registry[8].name, "security_disclose");
    }

    /// Calling an unknown tool is a tool error, not a protocol error.
    #[test]
    fn unknown_tool_is_an_error() {
        let outcome = SecurityServer::call("nope", &serde_json::json!({}));
        let err = outcome.unwrap_err();
        assert!(err.contains("unknown tool"), "unexpected error: {err}");
    }

    /// `security_evidence` without a run id reports the missing parameter.
    #[test]
    fn evidence_without_run_id_is_an_error() {
        let outcome = SecurityServer::call("security_evidence", &serde_json::json!({}));
        let err = outcome.unwrap_err();
        assert!(err.contains("run_id"), "unexpected error: {err}");
    }
}
