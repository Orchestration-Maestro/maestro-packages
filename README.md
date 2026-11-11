# Maestro packages

Build and share extensions, plugins and resource packages for
[Maestro](https://github.com/Orchestration-Maestro/maestro). This repository
holds community extensions and Maestro's examples, in every language; the
[examples guide](docs/examples.md) covers their layout, the Maestro pin and how
to run their tests. Start with the
[marketplace author guide](docs/marketplace-author-guide.md) to build and install
your first extension. The [marketplace design](docs/marketplace.md) covers
publishing, third-party review, catalog metadata and package management.
The [repository checks](docs/conventions.md) keep the tracked sources to Rust.

| Crate | Description |
|---|---|
| `maestro-prompt-suffix` | Input hook that appends an instruction to user prompts. |
| `maestro-packages-conventions` | Checks that members inherit the workspace lints and that git tracks only Rust sources. |
| `maestro-security` | Unified security framework: one pipeline, five agents, extension and MCP surfaces. |
| `maestro-test-extension-host` | Test support that runs extension components in a Maestro executable against a scripted model. |

Until `maestro-extensions-wasm` is published on crates.io, the crates here depend on it and on Maestro's test crates through git dependencies pinned to one Maestro commit. The [examples guide](docs/examples.md#the-maestro-pin) explains how to move the pin.
