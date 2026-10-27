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

/// Baseline: empty artifact paths fail before contacting services.
#[test]
fn requires_path() {
    let params = Artifact {
        path: " \n\t".into(),
        model: None,
    };
    assert_eq!(reverse(&params).err().as_deref(), Some("path is required"));
}

/// Baseline: reports bound and normalize collected facts while retaining interpretation gaps.
#[test]
fn normalizes_and_bounds_artifact_facts() -> Result {
    let strings = (0..200)
        .map(|i| format!("  string-{i}-{}  ", "é".repeat(40)))
        .collect::<Vec<_>>()
        .join("\n");
    let symbols = (0..100)
        .map(|i| format!("symbol-{i}-{}", "界".repeat(20)))
        .collect::<Vec<_>>()
        .join("\n");
    let sections = (0..60)
        .map(|i| format!("section-{i}-{}", "é".repeat(30)))
        .collect::<Vec<_>>()
        .join("\n");
    case(
        "reverse::normalizes_and_bounds_artifact_facts",
        vec![
            Reply::exec("artifact: ELF\nextra diagnostic\n", 0),
            Reply::exec(&format!("\n{strings}"), 0),
            Reply::exec(&symbols, 0),
            Reply::exec(&sections, 0),
            Reply::chat(
                r#"{"analysis":"An executable with exported symbols","unknown":["runtime behaviour",9,null,"packing"]}"#,
            ),
        ],
        || {
            let report = reverse(&Artifact {
                path: "artifact".into(),
                model: Some("fixture-model".into()),
            })?;
            assert_eq!(report.path, "artifact");
            assert_eq!(report.model, "fixture-model");
            assert_eq!(report.file_type, "artifact: ELF");
            assert_eq!(report.strings.len(), 120);
            assert_eq!(report.strings[0], format!("string-0-{}", "é".repeat(40)));
            assert_eq!(
                report.strings[119],
                format!("string-119-{}", "é".repeat(40))
            );
            assert_eq!(report.symbols.len(), 80);
            assert_eq!(report.symbols[79], format!("symbol-79-{}", "界".repeat(20)));
            assert_eq!(report.sections.len(), 40);
            assert_eq!(
                report.sections[39],
                format!("section-39-{}", "é".repeat(30))
            );
            assert_eq!(report.analysis, "An executable with exported symbols");
            assert_eq!(report.unknown, ["runtime behaviour", "packing"]);
            Ok(())
        },
    )
}

/// Baseline: inspection failures leave empty facts and unstructured interpretation is preserved.
#[test]
fn failed_inspections_preserve_unstructured_analysis() -> Result {
    case(
        "reverse::failed_inspections_preserve_unstructured_analysis",
        vec![
            Reply::failure("/exec"),
            Reply::failure("/exec"),
            Reply::failure("/exec"),
            Reply::failure("/exec"),
            Reply::chat("Not enough facts to classify the artifact."),
        ],
        || {
            let report = reverse(&Artifact {
                path: "artifact".into(),
                model: Some("fixture-model".into()),
            })?;
            assert!(report.file_type.is_empty());
            assert!(report.strings.is_empty());
            assert!(report.symbols.is_empty());
            assert!(report.sections.is_empty());
            assert_eq!(
                report.analysis,
                "Not enough facts to classify the artifact."
            );
            assert!(report.unknown.is_empty());
            Ok(())
        },
    )
}

/// Baseline: interpretation service failure is returned rather than reporting a completed analysis.
#[test]
fn propagates_interpretation_failure() -> Result {
    case(
        "reverse::propagates_interpretation_failure",
        vec![
            Reply::exec("", 0),
            Reply::exec("", 0),
            Reply::exec("", 0),
            Reply::exec("", 0),
            Reply::failure("/chat/completions"),
        ],
        || {
            let error = reverse(&Artifact {
                path: "artifact".into(),
                model: Some("fixture-model".into()),
            })
            .err()
            .ok_or("expected model error")?;
            assert!(error.starts_with("bridge http 403"));
            Ok(())
        },
    )
}
