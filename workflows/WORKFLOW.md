# maestro-packages workflow — security pipeline

My own workflow for this repository, following Maestro's behavior (Matt's flow
plus the workspace orchestration rules). Every file change stays in this repo.

## Roles and models

- **Supervisor** (this session): orchestrates, rules, verifies, integrates.
- **Subagents / lanes**: `deepseek/deepseek-flash` only (owner rule).
  - Review lanes (Standards axis and Spec axis): deepseek-flash, read-only.
  - Fix lanes: deepseek-flash.
- No Claude and no Sol for subagents in this repository.

## Stages (Matt's flow)

1. **Issue-first** — every change starts as a GitHub issue in this repository.
2. **Spec** — the issue points to the spec: `DECOMPOSITION-MATRIX.md`,
   `UNIFIED-DESIGN.md`, `SKILL.md`.
3. **Tickets** — one tracer-bullet ticket per bounded slice; the pipeline is
   one slice.
4. **TDD** — red-first tests prove real behaviour; no tautological tests.
5. **Verify** — AWS remote build only (no local builds):
   - `maestro-remote-build run cargo test -p maestro-http -p maestro-security`
   - `maestro-remote-build run cargo clippy -p maestro-http -p maestro-security --all-targets`
   - `RUSTDOCFLAGS="-D warnings -D missing_docs" maestro-remote-build run cargo doc -p maestro-security --no-deps`
6. **Two-axis review** — Standards (Rust idioms, quality) and Spec (matches the
   issue). Both on deepseek-flash, read-only.
7. **PR** — squash merge, conventional commit, signed.
8. **Retro** — what would prevent the next bug; mechanical mistakes become
   deterministic checks.

## Gates (all must pass before the PR)

- [ ] AWS test: `maestro-http` and `maestro-security` green.
- [ ] AWS clippy: 0 errors, 0 warnings (both crates, all-targets).
- [ ] AWS doc: clean.
- [ ] Two-axis review: no blocking findings.
- [ ] Issue #5 acceptance criteria met.

## Current state

- Issue: #5 (maestro-security is the sole assessment path).
- Branch: `feat/security-pipeline`.
- Pending in the working tree: `maestro-http` retry plus tests, `SKILL.md`,
  clippy fixes (let-else, map_or_else, serve_loop/serve_one restructure).
- Next: verify on AWS, commit, two-axis review (deepseek), PR, retro.
