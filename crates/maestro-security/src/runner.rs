//! Client for the Kali box runner service: the execution plane for every
//! command the pipeline generates.

use crate::net;
use serde_json::json;

/// Default runner base URL (host-side, loopback).
pub const DEFAULT_RUNNER_URL: &str = "http://127.0.0.1:8100";

/// One completed command execution from the runner.
#[derive(Debug, Clone)]
pub struct ExecResult {
    /// Combined stdout and stderr, in arrival order.
    pub output: String,
    /// Process exit code.
    pub exit_code: i32,
}

/// A configured runner client.
#[derive(Clone)]
pub struct Runner {
    /// Runner base URL.
    base_url: String,
}

impl Runner {
    /// Client pointed at the local runner defaults.
    #[must_use]
    pub fn local() -> Self {
        Self {
            base_url: DEFAULT_RUNNER_URL.into(),
        }
    }

    /// Override the base URL.
    #[must_use]
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Liveness check.
    ///
    /// # Errors
    /// Fails on any non-200 runner response.
    pub fn health(&self) -> Result<(), String> {
        let resp = net::request("GET", &format!("{}/health", self.base_url), &[], &[])?;
        if resp.status != 200 {
            return Err(format!("runner http {} on /health", resp.status));
        }
        Ok(())
    }

    /// Run one command and collect the NDJSON stream to a combined result.
    ///
    /// # Errors
    /// Fails on any non-200 runner response or a malformed NDJSON stream.
    pub fn exec(&self, command: &str, timeout_secs: u32) -> Result<ExecResult, String> {
        let body = json!({ "command": command, "timeout": timeout_secs }).to_string();
        let resp = net::request(
            "POST",
            &format!("{}/exec", self.base_url),
            &[("content-type", "application/json")],
            body.as_bytes(),
        )?;
        if resp.status != 200 {
            let text = String::from_utf8_lossy(&resp.body);
            return Err(format!("runner http {} on /exec: {}", resp.status, text));
        }
        parse_ndjson(&resp.body)
    }
}

/// Fold the runner's NDJSON stream into one combined result.
fn parse_ndjson(raw: &[u8]) -> Result<ExecResult, String> {
    let text = String::from_utf8_lossy(raw).to_string();
    let mut output = String::new();
    let mut exit_code = 0i32;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: serde_json::Value =
            serde_json::from_str(line).map_err(|e| format!("runner ndjson: {e}"))?;
        match value.get("type").and_then(|t| t.as_str()) {
            Some("stdout") | Some("stderr") => {
                if let Some(data) = value.get("data").and_then(|d| d.as_str()) {
                    output.push_str(data);
                }
            }
            Some("exit") => {
                let code = value.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
                exit_code = i32::try_from(code).unwrap_or(0);
            }
            // Tool banners and heartbeat lines carry no payload we need.
            _ => {}
        }
    }
    Ok(ExecResult { output, exit_code })
}
