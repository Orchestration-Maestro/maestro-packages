//! MCP stdio server for the maestro-security framework.
//!
//! Host-only: the WASM surface is the extension, not a stdio server.

#[cfg(not(target_family = "wasm"))]
fn main() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .map_err(|error| format!("runtime: {error}"))?;
    runtime.block_on(maestro_security::mcp::serve())
}

/// The no-op entry point for the wasm target, where the extension is the
/// surface.
#[cfg(target_family = "wasm")]
fn main() {}
