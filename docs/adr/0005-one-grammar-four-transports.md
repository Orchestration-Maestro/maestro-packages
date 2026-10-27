# One command grammar, four transports

The run surface is one grammar — `/maestro-security <verb>` — with verbs
assess, stop, steer, list, board, logs, report. Decided (2026-10-25): the
same grammar is exposed on all four Maestro transports: session
slash-commands, the maestro CLI, the MCP server, and the RPC/API surface.
One core, four access surfaces, no transport-specific grammar.
