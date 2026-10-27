//! Baseline source discovery and model triage through the public API.

use maestro_security::sourcehunt::{Query, hunt};

use super::support::{Reply, Result, case};

/// Baseline: blank paths are rejected before discovery.
#[test]
fn requires_path() {
    let params = Query {
        path: " \n".into(),
        ..Query::default()
    };
    assert_eq!(hunt(&params).err().as_deref(), Some("path is required"));
}

/// Baseline: discovery preserves source locations and triage drops false positives.
#[test]
fn triages_hits_without_losing_locations() -> Result {
    case(
        "sourcehunt::triages_hits_without_losing_locations",
        vec![
            Reply::exec(
                "src/a.rs:7:  eval(input: value)  \nmalformed\nsrc/b.rs:no:invalid\nsrc/b.rs:8: example\nsrc/c.rs:9: omitted\n",
                0,
            ),
            Reply::chat(
                r#"{"judgements":[{"index":0,"keep":true,"severity":"high","note":"user-controlled input"},{"index":1,"keep":false},{"index":99,"keep":true}]}"#,
            ),
        ],
        || {
            let report = hunt(&query())?;
            assert_eq!(report.path, "source tree");
            assert_eq!(report.hits.len(), 1);
            let hit = &report.hits[0];
            assert_eq!(hit.file, "src/a.rs");
            assert_eq!(hit.line, 7);
            assert_eq!(hit.pattern, "eval");
            assert_eq!(hit.text, "eval(input: value)");
            assert_eq!(hit.severity, "high");
            assert_eq!(hit.note, "user-controlled input");
            Ok(())
        },
    )
}

/// Baseline: no default-pattern hits returns an empty report without a model call.
#[test]
fn no_hits_skips_triage() -> Result {
    case(
        "sourcehunt::no_hits_skips_triage",
        (0..7).map(|_| Reply::exec("", 0)).collect(),
        || {
            let mut params = query();
            params.patterns.clear();
            assert!(hunt(&params)?.hits.is_empty());
            Ok(())
        },
    )
}

/// Baseline: malformed model output preserves bounded, untriaged evidence.
#[test]
fn malformed_triage_retains_raw_hits() -> Result {
    case(
        "sourcehunt::malformed_triage_retains_raw_hits",
        vec![
            Reply::exec(&format!("src/a.rs:12: {}\n", "界".repeat(220)), 0),
            Reply::chat("not JSON"),
        ],
        || {
            let report = hunt(&query())?;
            assert_eq!(report.hits.len(), 1);
            let hit = &report.hits[0];
            assert_eq!(hit.file, "src/a.rs");
            assert_eq!(hit.line, 12);
            assert_eq!(hit.text, "界".repeat(200));
            assert_eq!(hit.severity, "info");
            assert!(hit.note.is_empty());
            Ok(())
        },
    )
}

/// Baseline: missing judgement fields use informational defaults.
#[test]
fn missing_judgement_fields_default_safely() -> Result {
    case(
        "sourcehunt::missing_judgement_fields_default_safely",
        vec![
            Reply::exec("a.rs:1: eval(input)\n", 0),
            Reply::chat(r#"{"judgements":[{"index":0}]}"#),
        ],
        || {
            let report = hunt(&query())?;
            assert_eq!(report.hits.len(), 1);
            assert_eq!(report.hits[0].severity, "info");
            assert!(report.hits[0].note.is_empty());
            Ok(())
        },
    )
}

/// Baseline: runner and model service errors reach the caller.
#[test]
fn propagates_service_failures() -> Result {
    case(
        "sourcehunt::propagates_service_failures",
        vec![
            Reply::failure("/exec"),
            Reply::exec("a.rs:1: eval(input)\n", 0),
            Reply::failure("/chat/completions"),
        ],
        || {
            let discovery = hunt(&query()).err().ok_or("expected discovery error")?;
            assert!(discovery.starts_with("sourcehunt grep: runner http 403"));
            let triage = hunt(&query()).err().ok_or("expected triage error")?;
            assert!(triage.starts_with("bridge http 403"));
            Ok(())
        },
    )
}

/// A custom-pattern source request with a fixture model.
fn query() -> Query {
    Query {
        path: "source tree".into(),
        patterns: vec!["eval".into()],
        model: Some("fixture-model".into()),
    }
}
