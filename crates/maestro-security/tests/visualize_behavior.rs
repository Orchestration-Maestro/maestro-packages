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
