//! The MCP stdio surface: the same two tools as JSON-RPC 2.0 over stdio.
//!
//! Implements the MCP tool-server subset: initialize, tools/list,
//! tools/call, ping. Line-delimited JSON, one request per line.

use std::io::{self, BufRead, Write};

use serde_json::{json, Value};

use crate::pipeline::{assess, evidence_for, AssessParams};
use crate::tools;

/// The protocol version this server speaks.
const PROTOCOL_VERSION: &str = "2025-06-18";

/// Serve MCP over stdio until the client closes the stream.
pub fn serve() -> Result<(), String> {
    let stdin = io::stdin();
    let mut out = io::stdout();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(_) => break, // client closed the stream
        };
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(error) => {
                write_error(&mut out, &Value::Null, -32700, &format!("parse error: {error}"))?;
                continue;
            }
        };
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let is_notification = request.get("id").is_none();
        match method {
            "initialize" => write_result(
                &mut out,
                &id,
                &json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": "maestro-security",
                        "version": env!("CARGO_PKG_VERSION"),
                    }
                }),
            )?,
            "tools/list" => {
                if !is_notification {
                    write_result(&mut out, &id, &json!({ "tools": tool_list() }))?;
                }
            }
            "tools/call" => {
                if !is_notification {
                    write_result(&mut out, &id, &call_tool(&request))?;
                }
            }
            "ping" => {
                if !is_notification {
                    write_result(&mut out, &id, &json!({}))?;
                }
            }
            _ => {
                if !is_notification {
                    write_error(
                        &mut out,
                        &id,
                        -32601,
                        &format!("method not found: {method}"),
                    )?;
                }
            }
        }
    }
    Ok(())
}

/// The two tools, with their shared schemas.
fn tool_list() -> Value {
    json!([
        {
            "name": tools::ASSESS,
            "description": tools::ASSESS_DESCRIPTION,
            "inputSchema": tools::assess_schema(),
        },
        {
            "name": tools::EVIDENCE,
            "description": tools::EVIDENCE_DESCRIPTION,
            "inputSchema": tools::evidence_schema(),
        },
    ])
}

/// Dispatch one tools/call request; tool errors are results, not protocol errors.
fn call_tool(request: &Value) -> Value {
    let name = request
        .pointer("/params/name")
        .and_then(|n| n.as_str())
        .unwrap_or("");
    let arguments = request
        .pointer("/params/arguments")
        .cloned()
        .unwrap_or(Value::Null);
    let outcome = match name {
        tools::ASSESS => run_assess(&arguments),
        tools::EVIDENCE => run_evidence(&arguments),
        other => Err(format!("unknown tool: {other}")),
    };
    match outcome {
        Ok(text) => json!({
            "content": [{ "type": "text", "text": text }],
            "isError": false,
        }),
        Err(message) => json!({
            "content": [{ "type": "text", "text": message }],
            "isError": true,
        }),
    }
}

/// One assessment run, returned as the report JSON.
fn run_assess(arguments: &Value) -> Result<String, String> {
    let params: AssessParams =
        serde_json::from_value(arguments.clone()).map_err(|error| error.to_string())?;
    let report = assess(&params).map_err(|error| format!("assessment failed: {error}"))?;
    serde_json::to_string(&report).map_err(|error| error.to_string())
}

/// The retained evidence ledger for one finished run.
fn run_evidence(arguments: &Value) -> Result<String, String> {
    let run_id = arguments
        .get("run_id")
        .and_then(|v| v.as_str())
        .ok_or("run_id is required")?;
    evidence_for(run_id).map_err(|error| format!("evidence lookup failed: {error}"))
}

/// Write one JSON-RPC result line.
fn write_result(out: &mut impl Write, id: &Value, result: &Value) -> Result<(), String> {
    let message = json!({ "jsonrpc": "2.0", "id": id, "result": result });
    writeln!(out, "{message}").map_err(|error| format!("stdout: {error}"))?;
    out.flush().map_err(|error| format!("stdout: {error}"))
}

/// Write one JSON-RPC error line.
fn write_error(out: &mut impl Write, id: &Value, code: i64, message: &str) -> Result<(), String> {
    let response = json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    });
    writeln!(out, "{response}").map_err(|error| format!("stdout: {error}"))?;
    out.flush().map_err(|error| format!("stdout: {error}"))
}

#[cfg(test)]
mod tests {
    //! Unit tests for the MCP surface.

    use super::{call_tool, tool_list};
    use serde_json::json;

    /// `tools/list` exposes exactly the two shared tools.
    #[test]
    fn tool_list_has_both_tools() {
        let tools = tool_list();
        let arr = tools.as_array().expect("array");
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["name"], "security_assess");
        assert_eq!(arr[1]["name"], "security_evidence");
    }

    /// Calling an unknown tool is a tool error, not a protocol error.
    #[test]
    fn unknown_tool_is_an_error_result() {
        let request = json!({ "params": { "name": "nope", "arguments": {} } });
        let result = call_tool(&request);
        assert_eq!(result["isError"], true);
    }

    /// `security_evidence` without a run id reports the missing parameter.
    #[test]
    fn evidence_without_run_id_is_an_error_result() {
        let request = json!({ "params": { "name": "security_evidence", "arguments": {} } });
        let result = call_tool(&request);
        assert_eq!(result["isError"], true);
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("run_id"));
    }
}
