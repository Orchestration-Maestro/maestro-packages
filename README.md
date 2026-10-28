# Maestro packages

Build and share extensions, plugins and resource packages for
[Maestro](https://github.com/Orchestration-Maestro/maestro). Start with the
[marketplace author guide](docs/marketplace-author-guide.md) to build and install
your first extension. The [marketplace design](docs/marketplace.md) covers
publishing, third-party review, catalog metadata and package management.
See the [TypeScript and Python guest status](examples/README.md) for the pinned
WIT world and current verification blockers.

| Crate | Description |
|---|---|
| `maestro-prompt-suffix` | Input hook that appends an instruction to user prompts. |
| `maestro-security` | Unified security framework: one pipeline, five agents, extension and MCP surfaces. |

Until `maestro-extensions-wasm` is published on crates.io, extension crates depend on it through the maestro repository's `main` branch.
