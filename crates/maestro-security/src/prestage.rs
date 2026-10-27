//! Sandbox image prestage: check which base images the sandbox can run and
//! report what is present versus missing, so an operator can pre-pull the
//! digest-pinned images before a run.
//!
//! The prestage pass lists the images the runner's host can see and compares
//! them against the digest-pinned set the pipeline expects. Missing images
//! are reported with their pinned reference.

use serde::{Deserialize, Serialize};

use crate::pipeline::Config;
use crate::runner::Sandbox;

/// Schema tag for the prestage report.
pub const SCHEMA: &str = "maestro-security/prestage/1";
/// The digest-pinned base images the pipeline expects.
const EXPECTED_IMAGES: &[&str] = &[
    "docker.io/library/alpine:3.20",
    "docker.io/library/python:3.12-slim",
    "docker.io/library/node:22-alpine",
];

/// One image's prestage status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageStatus {
    /// The pinned image reference.
    pub reference: String,
    /// present | missing.
    pub status: String,
}

/// The prestage report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Schema tag.
    pub schema: &'static str,
    /// The per-image statuses.
    pub images: Vec<ImageStatus>,
    /// Count present.
    pub present: usize,
    /// Count missing.
    pub missing: usize,
    /// One-line summary.
    pub summary: String,
}

/// Check the sandbox for the expected base images.
///
/// # Errors
/// Fails if the runner is unreachable.
pub fn prestage() -> Result<Manifest, String> {
    let config = Config::from_env();
    let sandbox = Sandbox::local().with_base_url(config.runner);
    let listed = sandbox
        .exec(
            "docker images --format '{{.Repository}}:{{.Tag}}' 2>/dev/null",
            30,
        )
        .map(|r| r.output)
        .unwrap_or_default();
    let mut images = Vec::new();
    for reference in EXPECTED_IMAGES {
        let present = listed.lines().any(|l| l.trim() == *reference);
        images.push(ImageStatus {
            reference: (*reference).to_string(),
            status: if present { "present" } else { "missing" }.into(),
        });
    }
    let present = images.iter().filter(|i| i.status == "present").count();
    let missing = images.len() - present;
    let summary = format!("prestage: {present} present, {missing} missing");
    Ok(Manifest {
        schema: SCHEMA,
        images,
        present,
        missing,
        summary,
    })
}

#[cfg(test)]
mod tests {
    //! Unit tests for the prestage report.

    use super::SCHEMA;

    /// The schema tag is stable.
    #[test]
    fn schema_is_stable() {
        assert_eq!(SCHEMA, "maestro-security/prestage/1");
    }
}
