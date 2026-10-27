//! Call-graph visualization: extract function definitions and call sites from
//! a source tree through the sandbox, build a directed call graph, and render
//! it as Mermaid for interactive viewing.

use std::fmt::Write;

use serde::{Deserialize, Serialize};

use crate::ctx::CapabilityCtx;
use crate::pipeline::Config;

/// Schema tag for the call graph.
pub const SCHEMA: &str = "maestro-security/visualize/1";

/// Parameters for a call-graph build.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Trace {
    /// Path to the source tree or file.
    pub path: String,
    /// Model override (unused for the deterministic graph; kept for parity).
    #[serde(default)]
    pub model: Option<String>,
}

/// One node in the call graph (a function).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallNode {
    /// The function name.
    pub name: String,
    /// The file it is defined in, if known.
    pub file: String,
    /// The line it is defined on, if known.
    pub line: u32,
}

/// One directed edge (a call).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallEdge {
    /// The caller.
    pub from: String,
    /// The callee.
    pub to: String,
    /// The file the call appears in.
    pub file: String,
}

/// The call graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallGraph {
    /// Schema tag.
    pub schema: &'static str,
    /// The path analyzed.
    pub path: String,
    /// The function nodes.
    pub nodes: Vec<CallNode>,
    /// The call edges.
    pub edges: Vec<CallEdge>,
    /// A Mermaid `graph TD` rendering.
    pub mermaid: String,
}

/// Build the call graph for a source tree.
///
/// # Errors
/// Fails if the path is empty or the sandbox call fails.
pub fn visualize(params: &Trace) -> Result<CallGraph, String> {
    if params.path.trim().is_empty() {
        return Err("path is required".into());
    }
    let ctx = CapabilityCtx::build(&Config::from_env(), params.model.as_deref())?;
    let definitions = discover_definitions(&ctx, &params.path)?;
    let edges = discover_calls(&ctx, &params.path, &definitions);
    let nodes: Vec<CallNode> = definitions
        .iter()
        .map(|(name, file, line)| CallNode {
            name: name.clone(),
            file: file.clone(),
            line: *line,
        })
        .collect();
    Ok(CallGraph {
        schema: SCHEMA,
        path: params.path.clone(),
        mermaid: render_mermaid(&nodes, &edges),
        nodes,
        edges,
    })
}

/// Discover function definitions as `(name, file, line)` triples.
fn discover_definitions(
    ctx: &CapabilityCtx,
    path: &str,
) -> Result<Vec<(String, String, u32)>, String> {
    let command = format!(
        "grep -rnE -- '^[[:space:]]*(pub[[:space:]]+)?(fn|func|def)[[:space:]]+[a-zA-Z_][a-zA-Z0-9_]*' {} 2>/dev/null | head -200",
        shell_quote(path)
    );
    let result = ctx
        .sandbox
        .exec(&command, 60)
        .map_err(|e| format!("visualize defs: {e}"))?;
    Ok(parse_definitions(&result.output))
}

/// Discover call edges between the discovered functions.
fn discover_calls(
    ctx: &CapabilityCtx,
    path: &str,
    definitions: &[(String, String, u32)],
) -> Vec<CallEdge> {
    let mut edges = Vec::new();
    for def in definitions {
        let command = format!(
            "grep -rnE -- '\\b{}\\s*\\(' {} 2>/dev/null | grep -v 'fn {}' | grep -v 'func {}' | grep -v 'def {}' | head -20",
            shell_quote(&def.0),
            shell_quote(path),
            def.0,
            def.0,
            def.0
        );
        let Ok(result) = ctx.sandbox.exec(&command, 60) else {
            continue;
        };
        for line in result.output.lines() {
            let file = line
                .split_once(':')
                .map(|(f, _)| f.to_string())
                .unwrap_or_default();
            if file.is_empty() {
                continue;
            }
            let to_name = def.0.clone();
            // The caller is the nearest enclosing definition we can attribute;
            // we record the edge from the file's module name as a coarse caller.
            let from_name = caller_for(&file);
            edges.push(CallEdge {
                from: from_name,
                to: to_name,
                file,
            });
        }
    }
    edges
}

/// Parse `file:line:definition` grep lines into `(name, file, line)`.
fn parse_definitions(output: &str) -> Vec<(String, String, u32)> {
    let mut out = Vec::new();
    for line in output.lines() {
        let Some((file, rest)) = line.split_once(':') else {
            continue;
        };
        let Some((line_no, code)) = rest.split_once(':') else {
            continue;
        };
        let Ok(line) = line_no.trim().parse::<u32>() else {
            continue;
        };
        let name = extract_name(code);
        if !name.is_empty() {
            out.push((name, file.to_string(), line));
        }
    }
    out
}

/// Extract the function name from a definition line.
fn extract_name(code: &str) -> String {
    code.split_whitespace()
        .find_map(|token| {
            let name = token
                .trim_end_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .next()
                .unwrap_or_default();
            let is_name = !name.is_empty()
                && name.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                && !matches!(name, "pub" | "fn" | "func" | "def" | "async");
            is_name.then(|| name.to_string())
        })
        .unwrap_or_default()
}

/// A coarse caller label from a file path (its file stem).
fn caller_for(file: &str) -> String {
    let name = file.rsplit('/').next().unwrap_or(file);
    name.rsplit_once('.')
        .map_or(name, |(stem, _)| stem)
        .to_string()
}

/// Render the call graph as a Mermaid `graph TD` block.
fn render_mermaid(nodes: &[CallNode], edges: &[CallEdge]) -> String {
    let mut out = String::from("graph TD\n");
    for node in nodes {
        let _ = writeln!(out, "  {}[\"{}\"]", node.name, node.name);
    }
    for edge in edges {
        let _ = writeln!(out, "  {} --> {}", edge.from, edge.to);
    }
    out
}

/// Single-quote a string for safe shell interpolation.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    //! Unit tests for call-graph helpers.

    use super::{extract_name, parse_definitions};

    /// A definition line yields its function name.
    #[test]
    fn extracts_function_name() {
        assert_eq!(extract_name("pub fn main() {"), "main");
        assert_eq!(extract_name("def build():"), "build");
    }

    /// A grep definition block parses into triples.
    #[test]
    fn parses_definitions_block() {
        let out = "src/a.rs:3:pub fn alpha() {\nsrc/b.rs:9:fn beta() {";
        let defs = parse_definitions(out);
        assert_eq!(defs.len(), 2);
        assert_eq!(defs[0].0, "alpha");
        assert_eq!(defs[0].1, "src/a.rs");
        assert_eq!(defs[0].2, 3);
    }
}
