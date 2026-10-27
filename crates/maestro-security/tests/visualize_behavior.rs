//! Regression tests through the call-graph capability.
mod support;

use maestro_security::visualize::{Trace, visualize};

/// Calls are attributed to the source module, not its file extension.
#[test]
fn graph_attributes_calls_to_the_source_module() {
    if !support::runner(
        "graph_attributes_calls_to_the_source_module",
        vec![
            support::output("src/service.rs:3:pub fn handle() {\n"),
            support::output("src/client.rs:8: handle();\n"),
        ],
    ) {
        return;
    }
    let graph = visualize(&Trace {
        path: "src".into(),
        model: Some("fixture".into()),
    })
    .expect("call graph");
    assert_eq!(graph.nodes.len(), 1);
    assert_eq!(graph.nodes[0].name, "handle");
    assert_eq!(graph.nodes[0].file, "src/service.rs");
    assert_eq!(graph.nodes[0].line, 3);
    assert_eq!(graph.edges.len(), 1);
    assert_eq!(graph.edges[0].from, "client");
    assert_eq!(graph.edges[0].to, "handle");
    assert_eq!(graph.edges[0].file, "src/client.rs");
    assert!(graph.mermaid.contains("client --> handle"));
}

/// Baseline: blank paths are rejected before any bridge or runner work.
#[test]
fn baseline_graph_requires_a_nonblank_path() {
    assert_eq!(
        visualize(&Trace {
            path: " \t ".into(),
            model: Some("fixture".into())
        })
        .unwrap_err(),
        "path is required"
    );
}

/// Baseline: malformed discovery lines do not create phantom nodes or edges.
#[test]
fn baseline_graph_ignores_malformed_definitions() {
    if !support::runner(
        "baseline_graph_ignores_malformed_definitions",
        vec![support::output(
            "not a definition\nsrc/a.rs:bad:fn alpha() {\nsrc/a.rs:3\nsrc/a.rs:4:fn\n",
        )],
    ) {
        return;
    }
    let graph = visualize(&Trace {
        path: "src".into(),
        model: Some("fixture".into()),
    })
    .expect("empty graph");
    assert!(graph.nodes.is_empty());
    assert!(graph.edges.is_empty());
}

/// Baseline: discovery failures reach the caller instead of an empty success.
#[test]
fn baseline_graph_propagates_discovery_failure() {
    if !support::runner(
        "baseline_graph_propagates_discovery_failure",
        vec![(401, "runner denied".into())],
    ) {
        return;
    }
    let error = visualize(&Trace {
        path: "src".into(),
        model: Some("fixture".into()),
    })
    .unwrap_err();
    assert!(error.contains("visualize defs"));
    assert!(error.contains("401"));
}

/// Baseline: unavailable call-site discovery keeps definitions but invents no edges.
#[test]
fn baseline_graph_retains_definitions_when_call_discovery_fails() {
    if !support::runner(
        "baseline_graph_retains_definitions_when_call_discovery_fails",
        vec![
            support::output("src/service.rs:3:pub fn handle() {\n"),
            (401, "runner denied".into()),
        ],
    ) {
        return;
    }
    let graph = visualize(&Trace {
        path: "src".into(),
        model: Some("fixture".into()),
    })
    .expect("partial graph");
    assert_eq!(graph.nodes.len(), 1);
    assert_eq!(graph.nodes[0].name, "handle");
    assert!(graph.edges.is_empty());
}
