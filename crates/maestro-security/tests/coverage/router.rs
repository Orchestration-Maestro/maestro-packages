//! Routing behaviour through default, supplied and file-backed documents.

use maestro_security::{Domain, DomainsDoc, Router};
use std::path::Path;

/// A supplied domain whose mixed-case hint also tests case-insensitive matching.
fn domain(id: &str, hints: &[&str]) -> Domain {
    Domain {
        id: id.into(),
        name: id.into(),
        hints: hints.iter().map(|hint| (*hint).into()).collect(),
        primary: "fixture-capability".into(),
        topics: vec!["fixture-topic".into()],
    }
}

/// Rank by matched hints, not word occurrence count, with stable document ties.
#[test]
fn baseline_custom_routes_rank_hits_and_keep_first_tie() {
    let router = Router::from(DomainsDoc {
        schema: "maestro-security/domains/1".into(),
        domains: vec![
            domain("first", &["ALPHA"]),
            domain("second", &["beta", "gamma"]),
        ],
    });
    for (task, expected) in [
        ("alpha BETA", "first"),
        ("ALPHA alpha alpha beta GAMMA", "second"),
        ("prefixALPHAsuffix", "first"),
    ] {
        assert_eq!(
            router.route(task).map(|domain| domain.id.as_str()),
            Some(expected)
        );
    }
    assert!(router.route("unmatched").is_none());
    assert!(router.route("").is_none());
    let empty = Router::from(DomainsDoc {
        schema: "custom".into(),
        domains: vec![],
    });
    assert!(empty.route("alpha").is_none());
}

/// Each embedded domain exposes its capability and topics to a selected caller.
#[test]
fn baseline_embedded_domains_dispatch_to_capabilities() {
    let router = Router::default_();
    for (task, id, primary, topic) in [
        ("http graphql xss", "web", "surface", "injection"),
        ("tcp udp banner", "network", "surface", "protocol-abuse"),
        (
            "binary elf disassemble",
            "binary",
            "reverse",
            "static-analysis",
        ),
        (
            "repository sink taint",
            "source",
            "sourcehunt",
            "sink-tracing",
        ),
        (
            "aws kubernetes terraform",
            "cloud",
            "audit",
            "misconfiguration",
        ),
        ("ldap kerberos saml", "identity", "exploit", "token-abuse"),
        ("ctf flag crypto", "ctf", "exploit", "forensics"),
    ] {
        let selected = router.route(task);
        assert_eq!(selected.map(|domain| domain.id.as_str()), Some(id));
        assert_eq!(
            selected.map(|domain| domain.primary.as_str()),
            Some(primary)
        );
        assert!(selected.is_some_and(|domain| domain.topics.iter().any(|value| value == topic)));
    }
}

/// File configuration replaces defaults; bad files retain usable routing.
#[test]
fn baseline_loads_custom_documents_and_falls_back_on_bad_files() -> super::support::TestResult {
    let directory = std::env::temp_dir().join(format!("maestro-router-{}", std::process::id()));
    std::fs::create_dir(&directory)?;
    let path = directory.join("domains.json");
    let result = check_file_routes(&path);
    let cleanup = std::fs::remove_file(&path);
    let cleanup_directory = std::fs::remove_dir(&directory);
    result?;
    cleanup?;
    cleanup_directory?;
    Ok(())
}

/// Exercise readable, missing, invalid and empty documents at the file boundary.
fn check_file_routes(path: &Path) -> super::support::TestResult {
    let document = serde_json::json!({"schema":"maestro-security/domains/1","domains":[
        {"id":"custom","name":"Custom","hints":["unique"],"primary":"audit"}
    ]});
    std::fs::write(path, document.to_string())?;
    let router = Router::load(path);
    assert_eq!(
        router.route("UNIQUE").map(|domain| domain.id.as_str()),
        Some("custom")
    );
    assert!(router.route("http").is_none());
    assert!(router.domains()[0].topics.is_empty());
    assert_eq!(router.domains()[0].name, "Custom");
    for raw in ["not json", r#"{"schema":"s","domains":[{}]}"#] {
        std::fs::write(path, raw)?;
        assert_eq!(
            Router::load(path)
                .route("http")
                .map(|domain| domain.id.clone()),
            Some("web".into())
        );
    }
    std::fs::write(path, r#"{"schema":"s","domains":[]}"#)?;
    assert!(Router::load(path).route("http").is_none());
    std::fs::remove_file(path)?;
    assert_eq!(
        Router::load(path)
            .route("http")
            .map(|domain| domain.id.clone()),
        Some("web".into())
    );
    std::fs::write(path, [0xff])?;
    assert_eq!(
        Router::load(path)
            .route("http")
            .map(|domain| domain.id.clone()),
        Some("web".into())
    );
    Ok(())
}
