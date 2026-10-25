//! Environment health checks: verify the pieces the system depends on are
//! reachable and configured — the runner, the bridge, the model catalog, the
//! sandbox tooling, and the hardware — and report each with a 4-way
//! classification and a remediation line.

use serde::{Deserialize, Serialize};

use crate::llm::Client;
use crate::pipeline::Config;
use crate::runner::Sandbox;

/// Schema tag for the doctor report.
pub const SCHEMA: &str = "maestro-security/doctor/2";

/// The 4-way classification of a check result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Classification {
    /// The dependency is present and working.
    Healthy,
    /// The dependency exists but the configuration points somewhere else.
    ConfigDrift,
    /// The dependency is absent from the environment.
    MissingAnalysisEngine,
    /// The host cannot run the dependency at all.
    UnsupportedHost,
}

/// One health check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    /// The check name.
    pub name: String,
    /// pass | fail.
    pub status: String,
    /// The 4-way classification.
    pub classification: Classification,
    /// A detail line.
    pub detail: String,
    /// What to do about it; empty when healthy.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub remediation: String,
}

/// The doctor report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vitals {
    /// Schema tag.
    pub schema: &'static str,
    /// The checks, in order.
    pub checks: Vec<HealthCheck>,
    /// Count of passes.
    pub passed: usize,
    /// Count of failures.
    pub failed: usize,
    /// One-line summary.
    pub summary: String,
}

/// Run every health check and report the results.
///
/// # Errors
/// Does not fail on individual check failures; it reports them. It only
/// fails if the configuration cannot be read.
pub fn doctor() -> Result<Vitals, String> {
    let config = Config::from_env();
    let checks = vec![
        check_runner(&config),
        check_bridge(&config),
        check_models(&config),
        check_tooling(&config),
        check_hardware(&config),
    ];
    let passed = checks.iter().filter(|c| c.status == "pass").count();
    let failed = checks.len() - passed;
    let summary = format!("doctor: {passed} passed, {failed} failed");
    Ok(Vitals {
        schema: SCHEMA,
        checks,
        passed,
        failed,
        summary,
    })
}

/// Check the runner is reachable.
fn check_runner(config: &Config) -> HealthCheck {
    let sandbox = Sandbox::local().with_base_url(config.runner.clone());
    match sandbox.health() {
        Ok(()) => ok("runner", &config.runner),
        Err(e) => drift("runner", &config.runner, &e, "point MAESTRO_SECURITY_RUNNER at the live runner"),
    }
}

/// Check the bridge is reachable.
fn check_bridge(config: &Config) -> HealthCheck {
    let client = Client::local()
        .with_base_url(config.bridge.clone())
        .with_key(config.bridge_key.clone());
    match client.available_models() {
        Ok(_) => ok("bridge", &config.bridge),
        Err(e) => drift("bridge", &config.bridge, &e, "point MAESTRO_SECURITY_BRIDGE at the live bridge"),
    }
}

/// Check the bridge serves at least one model.
fn check_models(config: &Config) -> HealthCheck {
    let client = Client::local()
        .with_base_url(config.bridge.clone())
        .with_key(config.bridge_key.clone());
    match client.available_models() {
        Ok(models) => ok(
            "models",
            &format!("{} models, default={}", models.len(), config.default_model),
        ),
        Err(e) => drift("models", "bridge /models", &e, "start the model router or fix the bridge key"),
    }
}

/// Check the sandbox has the core tooling the pipeline relies on.
fn check_tooling(config: &Config) -> HealthCheck {
    let sandbox = Sandbox::local().with_base_url(config.runner.clone());
    let probe = "command -v nmap && command -v grep && command -v find";
    match sandbox.exec(probe, 30) {
        Ok(r) if r.exit_code == 0 => ok("tooling", "nmap, grep, find present"),
        Ok(r) => HealthCheck {
            name: "tooling".into(),
            status: "fail".into(),
            classification: Classification::MissingAnalysisEngine,
            detail: format!("probe exited {}", r.exit_code),
            remediation: "install the missing tool in the sandbox image".into(),
        },
        Err(e) => drift("tooling", "probe", &e, "check the runner endpoint"),
    }
}

/// Check the hardware. A GPU is reported only when the probe proves a
/// usable device; an absent GPU is healthy cpu-only, never a failure.
fn check_hardware(config: &Config) -> HealthCheck {
    let sandbox = Sandbox::local().with_base_url(config.runner.clone());
    match sandbox.exec("nvidia-smi -L 2>/dev/null || echo none", 30) {
        Ok(r) if r.exit_code == 0 => {
            let devices: Vec<&str> = r
                .output
                .lines()
                .filter(|l| l.starts_with("GPU ") || l.trim() == "none")
                .collect();
            if devices.iter().any(|d| d.starts_with("GPU ")) {
                ok("hardware", &format!("{} gpu device(s)", devices.len()))
            } else {
                ok("hardware", "cpu-only")
            }
        }
        Ok(r) => HealthCheck {
            name: "hardware".into(),
            status: "fail".into(),
            classification: Classification::UnsupportedHost,
            detail: format!("hardware probe exited {}", r.exit_code),
            remediation: "run the sandbox on a host with working device access".into(),
        },
        Err(e) => drift("hardware", "probe", &e, "check the runner endpoint"),
    }
}

/// A passing check.
fn ok(name: &str, detail: &str) -> HealthCheck {
    HealthCheck {
        name: name.into(),
        status: "pass".into(),
        classification: Classification::Healthy,
        detail: detail.into(),
        remediation: String::new(),
    }
}

/// A failing check with a config-drift classification.
fn drift(name: &str, target: &str, error: &str, remediation: &str) -> HealthCheck {
    HealthCheck {
        name: name.into(),
        status: "fail".into(),
        classification: Classification::ConfigDrift,
        detail: format!("{target}: {error}"),
        remediation: remediation.into(),
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the doctor helpers.

    use super::{ok, Classification};

    /// ok is healthy with no remediation; the classification is serializable.
    #[test]
    fn sets_status_and_classification() {
        let check = ok("a", "d");
        assert_eq!(check.status, "pass");
        assert_eq!(check.classification, Classification::Healthy);
        assert!(check.remediation.is_empty());
        let json = serde_json::to_string(&check).unwrap();
        assert!(json.contains("\"healthy\""));
    }
}
