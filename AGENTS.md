# maestro-packages

The [workspace rules](https://github.com/Orchestration-Maestro/maestro/blob/main/AGENTS.md)
in the core repository's agent file are the single rule source. Load them in
full from `../AGENTS.md`, one directory up from this repository's root. They
win over this file, which adds only repository-specific lines. This repository
is the marketplace for community extensions.

## Repository-specific

- Community extensions only: capability and default-on packages live in the core repository; this repository ships optional, community-published packages.
- Every package is one crate with one job.
- Every issue is labelled `packages` and opens with "Repository: maestro-packages".

## CI contract

[Shared Rust CI v0.19.4](https://github.com/Orchestration-Maestro/maestro-rust-workflows/blob/81b6113453fd38ff9fb27a5af846882d8354a95f/README.md)
and its [caller checks](https://github.com/Orchestration-Maestro/maestro-rust-workflows/blob/81b6113453fd38ff9fb27a5af846882d8354a95f/docs/caller-checks.md)
define the required checks. The organization ruleset attaches CI to repositories
with `stack=rust`; do not add a caller workflow here. `.github/ci.toml` declares
this workspace's targets and test tools. `mise.toml` pins just and prek;
`rust-toolchain.toml` pins Rust. Run `just setup` to install the hooks.
The pre-commit hook runs `just pre-commit`: capture staged paths, format,
re-stage only captured files, then check their crates and reverse dependents.
