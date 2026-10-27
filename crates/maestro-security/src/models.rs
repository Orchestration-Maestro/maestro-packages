//! Role-to-model resolution: map each agent role to the model it will run on,
//! resolved at run time from the bridge catalog.
//!
//! The model is a value, not a binding: this module reads the bridge's
//! `/models` and reports which model each role resolves to, so an operator
//! can see the routing before a run. Swapping the bridge model changes the
//! resolution with no code change.

use serde::{Deserialize, Serialize};

use crate::agents::ROSTER;
use crate::llm::Client;
use crate::pipeline::Config;

/// Schema tag for the model map.
pub const SCHEMA: &str = "maestro-security/models/1";

/// One role's resolved model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleModel {
    /// The agent role name.
    pub role: String,
    /// The model it resolves to.
    pub model: String,
    /// Whether the role's default model is explicitly served by the bridge.
    pub explicit: bool,
}

/// The model map for all roles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMap {
    /// Schema tag.
    pub schema: &'static str,
    /// The models the bridge currently serves.
    pub available: Vec<String>,
    /// The default model the pipeline prefers.
    pub default_model: String,
    /// The per-role resolution.
    pub roles: Vec<RoleModel>,
}

/// Resolve the model for every agent role against the live bridge catalog.
///
/// # Errors
/// Fails if the bridge serves no models.
pub fn models() -> Result<ModelMap, String> {
    let config = Config::from_env();
    let client = Client::local()
        .with_base_url(config.bridge.clone())
        .with_key(config.bridge_key.clone());
    let available = client.available_models()?;
    let mut roles = Vec::new();
    for agent in ROSTER {
        let explicit = available.contains(&config.default_model);
        roles.push(RoleModel {
            role: agent.name.to_string(),
            model: config.default_model.clone(),
            explicit,
        });
    }
    Ok(ModelMap {
        schema: SCHEMA,
        available,
        default_model: config.default_model,
        roles,
    })
}

