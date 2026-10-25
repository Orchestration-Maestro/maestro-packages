//! Shared context for capability modules: one resolved model, one LLM client
//! and one sandbox, built once per call so no capability re-resolves the
//! model or rebuilds its clients.

use crate::llm::Client;
use crate::pipeline::{resolve_model, Config};
use crate::runner::Sandbox;

/// The shared context a capability needs to run.
#[derive(Clone)]
pub struct CapabilityCtx {
    /// The bridge LLM client.
    pub client: Client,
    /// The analysis sandbox runner.
    pub sandbox: Sandbox,
    /// The model resolved at run time.
    pub model: String,
}

impl CapabilityCtx {
    /// Build a context from a configuration and an optional model override.
    ///
    /// # Errors
    /// Fails if the bridge serves no model.
    pub fn build(config: &Config, model_override: Option<&str>) -> Result<Self, String> {
        let client = Client::local()
            .with_base_url(config.bridge.clone())
            .with_key(config.bridge_key.clone());
        let model = resolve_model(&client, &config.default_model, model_override)?;
        let sandbox = Sandbox::local().with_base_url(config.runner.clone());
        Ok(Self {
            client,
            sandbox,
            model,
        })
    }

    /// One chat completion with the resolved model, using the unified system
    /// prompt plus a capability-specific role line.
    ///
    /// # Errors
    /// Fails on any non-200 bridge response or an empty answer.
    pub fn chat(
        &self,
        role: &str,
        user: &str,
        max_tokens: u32,
    ) -> Result<String, String> {
        let system = format!("{}\n\nCAPABILITY ROLE:\n{}", crate::methodology::SYSTEM_PROMPT, role);
        self.client
            .chat(&self.model, &system, user, max_tokens)
            .map(|reply| reply.content)
    }
}
