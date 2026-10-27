//! Behavioural coverage for model calls, role resolution and task routing.

#[path = "coverage/llm.rs"]
mod llm;
#[path = "coverage/models.rs"]
mod models;
#[path = "coverage/router.rs"]
mod router;
#[cfg(test)]
pub mod support;
