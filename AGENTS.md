# maestro-packages

The workspace rules in `../AGENTS.md`, one directory up from this repository's
root, are the single rule source. Load them in full; they win over this file,
which adds only repository-specific lines. The
[core repository's agent file](https://github.com/Orchestration-Maestro/maestro/blob/main/AGENTS.md)
is a separate reference.

## Repository-specific

- This repository holds community extensions and Maestro's examples, in every language. Capability and default-on packages live in the core repository; the extensions here are optional, community-published packages.
- Every issue is labelled `packages` and opens with "Repository: maestro-packages".
- Issue tracker: see `docs/agents/issue-tracker.md`.
- Triage labels: see `docs/agents/triage-labels.md`.
- Domain documentation: see `docs/agents/domain.md`.

## CI contract

[Shared Rust CI v0.19.4](https://github.com/Orchestration-Maestro/maestro-rust-workflows/blob/81b6113453fd38ff9fb27a5af846882d8354a95f/README.md)
and its [caller checks](https://github.com/Orchestration-Maestro/maestro-rust-workflows/blob/81b6113453fd38ff9fb27a5af846882d8354a95f/docs/caller-checks.md)
define the required checks. The organization ruleset attaches CI to repositories
with `stack=rust`; do not add a caller workflow here. `.github/ci.toml` declares
this workspace's targets and test tools. `mise.toml` pins just and prek;
`rust-toolchain.toml` pins Rust. Run `just setup` to install the hooks.
The pre-commit hook runs `just pre-commit`, which installs Maestro's development tool
at the pinned commit and runs `development pre-commit`: capture staged paths,
format, re-stage only captured files, then check their crates with Clippy.
