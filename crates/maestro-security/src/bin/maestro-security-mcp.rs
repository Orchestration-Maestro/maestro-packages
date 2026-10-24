//! MCP stdio server for the maestro-security framework.

fn main() {
    if let Err(error) = maestro_security::mcp::serve() {
        eprintln!("maestro-security-mcp: {error}");
        std::process::exit(1);
    }
}
