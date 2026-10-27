# The pipeline is user-initiated only

Decided (2026-10-25): a run starts only from a user-initiated request —
`/maestro-security`, the CLI, MCP or RPC. The Maestro agent may invoke it
as a tool when the user asks it to; nothing runs without the user's
request.
