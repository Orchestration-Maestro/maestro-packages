//! Reverse-report regressions through the public artifact API.

use maestro_security::reverse::{Artifact, reverse};

use super::support::{Reply, Result, case};

/// Regression: repeated non-adjacent artifact strings appear only once, in discovery order.
#[test]
fn unique_artifact_strings() -> Result {
    case(
        "reverse::unique_artifact_strings",
        vec![
            Reply::exec("artifact: ELF\n", 0),
            Reply::exec("alpha\nbeta\nalpha\ngamma\nbeta\n", 0),
            Reply::exec("", 0),
            Reply::exec("", 0),
            Reply::chat(r#"{"analysis":"Three distinct strings","unknown":[]}"#),
        ],
        || {
            let report = reverse(&Artifact {
                path: "artifact".into(),
                model: Some("fixture-model".into()),
            })?;
            assert_eq!(report.strings, ["alpha", "beta", "gamma"]);
            Ok(())
        },
    )
}
