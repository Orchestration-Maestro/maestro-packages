//! Isolated state and assessment fixtures for capability tests.
use std::path::Path;

use maestro_test_directory::Scratch;

/// Run with a fresh state directory, never the developer's retained evidence.
pub(crate) fn isolated(body: impl FnOnce(&Path)) {
    let home = Scratch::new().expect("fresh state directory");
    crate::evidence::with_home(home.path(), || body(home.path()));
}

/// Minimal completed assessment accepted by the retained report interface.
pub(crate) fn assessment() -> crate::report::Assessment {
    serde_json::from_value(serde_json::json!({
        "schema": crate::report::SCHEMA, "run_id": "run-test", "agent": "security",
        "target": "example.test", "model": "resolved", "phases": [], "findings": [{
            "id": "F-001", "fingerprint": "example-access", "title": "Access candidate",
            "class": "access", "verdict": "needs_validation", "severity": "low",
            "confidence": 0.5, "evidence_level": "LIKELY", "escalation": "none",
            "evidence": {"command": "probe", "output": "response", "id": "ev-0001"},
            "impact": "potential read access", "fix": "check authorization", "target": "example.test"
        }],
        "unknown": [], "refuted": [], "summary": "One candidate requires validation."
    }))
    .expect("assessment fixture")
}
