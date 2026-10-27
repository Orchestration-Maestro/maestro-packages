//! Model maps resolved against isolated bridge catalogs.

use super::support::{TestResult, bridge_url, captured};
use maestro_security::{ROSTER, models};

/// Resolve in a child process so environment overrides cannot race other tests.
#[test]
fn baseline_models_resolve_every_role_and_mark_availability() -> TestResult {
    if let Ok(case) = std::env::var("MAESTRO_TEST_MODEL_MAP_CASE") {
        if case == "error" {
            assert!(models::models().is_err_and(|error| error.contains("returned no models")));
            return Ok(());
        }
        let map = models::models()?;
        assert_eq!(map.available, ["catalog-first", "preferred"]);
        assert_eq!(map.default_model, case);
        assert_eq!(map.roles.len(), ROSTER.len());
        for (resolved, agent) in map.roles.iter().zip(ROSTER) {
            assert_eq!(resolved.role, agent.name);
            assert_eq!(
                resolved.model,
                if case == "preferred" {
                    "preferred"
                } else {
                    "catalog-first"
                }
            );
            assert_eq!(resolved.explicit, case == "preferred");
        }
        return Ok(());
    }
    for case in ["preferred", "unavailable", "error"] {
        let body = if case == "error" {
            r#"{"data":[]}"#
        } else {
            r#"{"data":[{"id":"catalog-first"},{"id":"preferred"}]}"#
        };
        let (request_url, server) = bridge_url(200, body)?;
        let output = std::process::Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "models::baseline_models_resolve_every_role_and_mark_availability",
                "--nocapture",
            ])
            .env("MAESTRO_TEST_MODEL_MAP_CASE", case)
            .env("MAESTRO_SECURITY_MODEL", case)
            .env("MAESTRO_SECURITY_BRIDGE", request_url)
            .env("MAESTRO_SECURITY_BRIDGE_KEY", "fixture-key")
            .output()?;
        let request = captured(server)?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(request.starts_with("GET /v1/models HTTP/1.1\r\n"));
        assert!(request.contains("\r\nauthorization: Bearer fixture-key\r\n"));
    }
    Ok(())
}
