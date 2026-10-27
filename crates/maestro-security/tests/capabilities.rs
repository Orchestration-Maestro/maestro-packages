//! Behavioural coverage through the browser and network capability surfaces.

#[cfg(test)]
pub mod support;

use maestro_security::{browser, surface};

/// Fetching a page retains its structure and the model's structured analysis.
#[test]
fn baseline_browser_captures_page_and_summary() {
    support::isolated(
        "baseline_browser_captures_page_and_summary",
        "fixture-model",
        &[r#"{"summary":"Login surface","unknown":["dynamic routes",7]}"#],
        |base| {
            let capture = browser::browse(&browser::Visit {
                url: format!("{base}/page"),
                model: None,
            })
            .unwrap();
            assert_eq!(capture.page.status, 200);
            assert_eq!(capture.page.url, format!("{base}/page"));
            assert_eq!(capture.page.title, "Login");
            assert_eq!(capture.page.links.len(), 1);
            assert_eq!(capture.page.links[0].href, "/login");
            assert_eq!(capture.page.links[0].text, "Sign in");
            assert_eq!(capture.page.forms, ["post /auth"]);
            assert_eq!(capture.page.inputs, ["user"]);
            assert_eq!(capture.summary, "Login surface");
            assert_eq!(capture.unknown, ["dynamic routes"]);
        },
    );
}

/// Plain-text model answers are retained and missing unknowns receive a warning.
#[test]
fn baseline_browser_preserves_unstructured_summary() {
    support::isolated(
        "baseline_browser_preserves_unstructured_summary",
        "fixture-override",
        &["Login page", r#"{"summary":"No scripts observed"}"#],
        |base| {
            let visit = browser::Visit {
                url: format!("{base}/page"),
                model: Some("fixture-override".into()),
            };
            let plain = browser::browse(&visit).unwrap();
            assert_eq!(plain.summary, "Login page");
            assert_eq!(plain.unknown, ["javascript-rendered content not captured"]);
            let structured = browser::browse(&visit).unwrap();
            assert_eq!(structured.summary, "No scripts observed");
            assert_eq!(structured.unknown, plain.unknown);
        },
    );
}

/// Missing inputs are rejected before any network request.
#[test]
fn baseline_capabilities_require_targets() {
    assert_eq!(
        browser::browse(&browser::Visit {
            url: " \n".into(),
            model: None
        })
        .unwrap_err(),
        "url is required"
    );
    assert_eq!(
        surface::dispatch(&surface::Probe::default()).unwrap_err(),
        "target is required"
    );
}

/// Discovery persists a baseline; monitoring reports changes and advances it.
#[test]
fn baseline_surface_monitors_retained_discoveries() {
    let first = r#"{"elements":[{"kind":"port","detail":"tcp/22","evidence":"ssh"},{"kind":"port","detail":"tcp/80"},{"detail":"invalid"}]}"#;
    let next = r#"```json
{"elements":[{"kind":"port","detail":"tcp/22","evidence":"new ssh proof"},{"kind":"port","detail":"tcp/443","evidence":"https"}]}
```"#;
    support::isolated(
        "baseline_surface_monitors_retained_discoveries",
        "fixture-model",
        &[first, next, next],
        |_| {
            let mut probe = surface::Probe {
                target: "example.test".into(),
                action: "discover".into(),
                model: None,
            };
            let discovered = surface::dispatch(&probe).unwrap();
            assert!(discovered.diff.is_none());
            let inventory = discovered.surface.unwrap();
            assert_eq!(inventory.target, "example.test");
            assert!(inventory.discovered_at > 0);
            assert_eq!(inventory.elements.len(), 2);
            assert_eq!(inventory.elements[1].evidence, "");
            probe.action = "monitor".into();
            let delta = surface::dispatch(&probe).unwrap().diff.unwrap();
            assert_eq!(delta.unchanged, 1);
            assert_eq!(delta.added.len(), 1);
            assert_eq!(delta.added[0].detail, "tcp/443");
            assert_eq!(delta.removed.len(), 1);
            assert_eq!(delta.removed[0].detail, "tcp/80");
            let repeated = surface::dispatch(&probe).unwrap().diff.unwrap();
            assert_eq!(repeated.unchanged, 2);
            assert!(repeated.added.is_empty());
            assert!(repeated.removed.is_empty());
        },
    );
}

/// A first monitor treats every discovered element as new; malformed output is empty.
#[test]
fn baseline_surface_without_baseline_or_structured_output() {
    support::isolated(
        "baseline_surface_without_baseline_or_structured_output",
        "fixture-override",
        &[
            r#"{"elements":[{"kind":"service","detail":"ssh"}]}"#,
            "not json",
            "{}",
        ],
        |_| {
            let probe = surface::Probe {
                target: "example.test".into(),
                action: "monitor".into(),
                model: Some("fixture-override".into()),
            };
            let initial = surface::dispatch(&probe).unwrap().diff.unwrap();
            assert_eq!(initial.added.len(), 1);
            assert_eq!(initial.added[0].kind, "service");
            assert_eq!(initial.unchanged, 0);
            assert!(initial.removed.is_empty());
            let invalid = surface::dispatch(&probe).unwrap().diff.unwrap();
            assert_eq!(invalid.removed.len(), 1);
            assert!(invalid.added.is_empty());
            let empty = surface::dispatch(&probe).unwrap().surface.unwrap();
            assert!(empty.elements.is_empty());
        },
    );
}
