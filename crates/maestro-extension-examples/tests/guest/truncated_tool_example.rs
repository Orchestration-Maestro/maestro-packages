//! The shipped search tool loaded into the production engine.
//!
//! Running the search itself needs a controlled `rg` process; the executable tests of this tool
//! start the application's native process owner against one, so these tests cover the
//! registration and the displays.
use super::support::{Shown, harness, load, local, only_tool_metadata, render_call, render_result};
use serde_json::{Value, json};

/// The variable that names the built search component.
const COMPONENT: &str = "MAESTRO_TRUNCATED_TOOL_COMPONENT";
/// The call every display below is drawn for.
fn call_args() -> Value {
    json!({"pattern": "needle", "path": "src", "glob": "*.rs"})
}

/// The tool registers with its search schema and the host's output limits in its description.
#[test]
fn truncated_tool_registers_search_schema_and_limits() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        assert_eq!(
            only_tool_metadata(&loaded.runner),
            json!({
                "name": "rg",
                "label": "ripgrep",
                "description": "Search file contents using ripgrep. Output is truncated to 2000 lines or 50.0KB (whichever is hit first). If truncated, full output is saved to a temp file.",
                "parameters": {"type": "object", "required": ["pattern"], "properties": {
                    "pattern": {"type": "string", "description": "Search pattern (regex)"},
                    "path": {"type": "string", "description": "Directory to search (default: current directory)"},
                    "glob": {"type": "string", "description": "File glob pattern, e.g. '*.ts'"}}}
            })
        );
    });
}

/// The call shows the pattern, directory and glob; the result shows its state and, expanded, its
/// lines, the truncation notice and where the full output went.
#[test]
fn truncated_tool_renders_calls_and_results() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let runner = &loaded.runner;
        assert_eq!(
            render_call(runner, "rg", &call_args(), 200),
            ["rg \"needle\" in src --glob *.rs"]
        );
        let shown = |result: &Value, [expanded, partial]: [bool; 2]| {
            let args = call_args();
            render_result(
                runner,
                "rg",
                &Shown {
                    expanded,
                    partial,
                    ..Shown::new(&args, result)
                },
            )
        };
        let empty = json!({"content": []});
        assert_eq!(shown(&empty, [false, true]), ["Searching..."]);
        assert_eq!(shown(&empty, [false; 2]), ["No matches found"]);
        let found = json!({"content": [{"type": "text", "text": "one\ntwo"}], "details": {"matchCount": 3}});
        assert_eq!(shown(&found, [false; 2]), ["3 matches"]);
        assert_eq!(shown(&found, [true, false]), ["3 matches", "one", "two"]);
        let text = (1..=21)
            .map(|line| format!("line {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let truncated = json!({"content": [{"type": "text", "text": text}], "details": {
            "matchCount": 3, "truncation": {"truncated": true},
            "fullOutputPath": "/tmp/maestro-rg-test/output.txt"}});
        assert_eq!(shown(&truncated, [false; 2]), ["3 matches (truncated)"]);
        let mut expanded = vec!["3 matches (truncated)".to_owned()];
        expanded.extend((1..=20).map(|line| format!("line {line}")));
        expanded.push("... (use read tool to see full output)".into());
        expanded.push("Full output: /tmp/maestro-rg-test/output.txt".into());
        assert_eq!(shown(&truncated, [true, false]), expanded);
    });
}
