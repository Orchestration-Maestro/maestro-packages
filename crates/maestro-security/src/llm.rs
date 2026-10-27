//! LLM client through the local bridge.
//!
//! The bridge is the single source of truth for the selected model: the
//! extension asks `/models` at run time and uses whatever the bridge serves,
//! so swapping models never touches this code. A per-run override is still
//! possible via the tool parameter.

use serde_json::json;

use crate::pipeline::truncate;

/// Default bridge base URL (loopback, WASM-visible): the local model router,
/// which serves every registered model directly, no capability key needed.
pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8080/v1";
/// Default capability key (machine-local; also in /work/tools/pi-bridge.env).
pub const DEFAULT_KEY: &str =
    "3648e45e792d925d2bf138156fb820e491a5f89f3c5d33f2d6963920b170592f";

/// A configured bridge client. The model is never stored here: it is resolved
/// per run from [`Client::available_models`] or supplied by the caller.
#[derive(Clone)]
pub struct Client {
    /// Bridge base URL including the audience segment.
    base_url: String,
    /// Bridge capability key.
    key: String,
}

impl Client {
    /// Client pointed at the local bridge defaults.
    #[must_use]
    pub fn local() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.into(),
            key: DEFAULT_KEY.into(),
        }
    }

    /// Override the base URL (audience or host).
    #[must_use]
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Override the capability key.
    #[must_use]
    pub fn with_key(mut self, key: impl Into<String>) -> Self {
        self.key = key.into();
        self
    }

    /// Models the bridge currently serves, in bridge order.
    ///
    /// # Errors
    /// Fails on any non-200 bridge response, malformed JSON, or an empty
    /// model list.
    pub fn available_models(&self) -> Result<Vec<String>, String> {
        let url = format!("{}/models", self.base_url);
        let resp = maestro_http::request(
            "GET",
            &url,
            &[("authorization", &format!("Bearer {}", self.key))],
            &[],
        )?;
        if resp.status != 200 {
            return Err(format!("bridge http {} on /models", resp.status));
        }
        let value: serde_json::Value =
            serde_json::from_slice(&resp.body).map_err(|e| format!("bridge json: {e}"))?;
        let ids = value
            .pointer("/data")
            .and_then(|d| d.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m.get("id")?.as_str().map(String::from))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if ids.is_empty() {
            return Err("bridge /models returned no models".into());
        }
        Ok(ids)
    }

    /// One non-streaming chat completion; returns the assistant message text.
    ///
    /// # Errors
    /// Fails on any non-200 bridge response, malformed JSON, or a response
    /// without message content.
    pub fn chat(
        &self,
        model: &str,
        system: &str,
        user: &str,
        max_tokens: u32,
    ) -> Result<ChatReply, String> {
        let body = json!({
            "model": model,
            "max_tokens": max_tokens,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
        })
        .to_string();
        let url = format!("{}/chat/completions", self.base_url);
        let resp = maestro_http::request(
            "POST",
            &url,
            &[
                ("authorization", &format!("Bearer {}", self.key)),
                ("content-type", "application/json"),
            ],
            body.as_bytes(),
        )?;
        if resp.status != 200 {
            let text = String::from_utf8_lossy(&resp.body);
            return Err(format!("bridge http {}: {}", resp.status, truncate(&text, 500)));
        }
        let value: serde_json::Value =
            serde_json::from_slice(&resp.body).map_err(|e| format!("bridge json: {e}"))?;
        let content = value
            .pointer("/choices/0/message/content")
            .and_then(|c| c.as_str())
            .unwrap_or_default()
            .to_string();
        if content.trim().is_empty() {
            // Thinking models spend tokens on reasoning before answering; an
            // empty answer means the budget ran out mid-thought, which the
            // phase layer retries.
            return Err("bridge returned no message content".into());
        }
        let tokens_out = value
            .pointer("/usage/completion_tokens")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        Ok(ChatReply {
            content,
            tokens_out,
        })
    }
}

/// One chat completion result: the content plus what it cost.
#[derive(Debug, Clone)]
pub struct ChatReply {
    /// The model's message content.
    pub content: String,
    /// Output tokens reported by the bridge; 0 when it reports none.
    pub tokens_out: u64,
}

