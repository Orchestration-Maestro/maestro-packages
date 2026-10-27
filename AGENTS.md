# maestro-packages

Extension packages for Maestro. This repository follows the same strict governance and structure as the maestro repository.

## Rules

- Issue first: every change starts with a GitHub issue in this repository.
- Every extension package lives here, never in the maestro core repository.
- One crate, one job; no duplicated code across crates (owner OD-413); one owner per piece of logic.
- Quality bar: workspace lints are `forbid` (pedantic, private docs, 5 params, 60-line functions, 500-line files, no unwrap/expect/panic outside tests); every public and private item is documented.
- Red-first tests prove real behaviour; no fake or tautological tests.
- Sign every commit; commit subject at most 71 characters; message lines at most 80 columns.
- Both reviews (standards and spec) pass before a PR merges; root-cause proof for every fix, before and after.
- Run the humanizer skill over every documentation file before landing and report the fixes.
- No dead weight: every line, test, abstraction, dependency and doc must be needed.
- Builds on the AWS remote-build host (`maestro-remote-build run ...`); never touch the local Qdrant or model router.
- No secrets in messages, tests or fixtures.

## Workflow (ask-matt pipeline)

- Every change follows Matt's flow: /grill-with-docs (when questions remain) → /to-spec → /to-tickets → /implement per ticket → /pr → /retro.
- A spec document lands on main before implementation: first commit of its ticket's PR; the issue points only at that one spec.
- Implement with /tdd, one red-green slice at a time; every change gets /code-review on both axes (Standards + Spec).
- Tickets carry acceptance criteria, commands and report format; a lane follows its ticket, never a private brief.
- Verify only on the AWS remote-build host (`maestro-remote-build run ...`); no local builds.
- Both reviews run read-only; blocking findings fixed before merge, minor doc-only findings become a cleanup issue.
- Merge with squash and a signed conventional commit; /retro after each ticket; mechanical mistakes become deterministic checks.
- Lane models: every lane, planning and review runs on deepseek/deepseek-flash; the supervisor names the model in each dispatch.
- Every issue is labelled `packages` and opens with "Repository: maestro-packages" so it never mixes with maestro core work.

## CI contract

[Shared Rust CI](https://github.com/Orchestration-Maestro/rust-workflows/blob/4a05e4c4c6cb82451af0b585255881bb7d860c81/README.md)
defines the caller in `.github/workflows/ci.yml` and its SARIF and coverage
upload jobs. CI checks the root workspace with `rust-toolchain.toml`, a 90%
coverage floor and the automatic license policy. Keep the caller's Rust version
in sync with the toolchain file. An administrator must register the shared
`Required Rust CI` job as a required check after this workflow lands.

## Agent skills

### Issue tracker

Issues live in GitHub Issues for this repo; every issue carries the `packages` label and a Repository stamp line. See `docs/agents/issue-tracker.md`.

### Triage labels

The five canonical roles: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: the glossary lives in `CONTEXT.md` (workspace convention), decisions in `docs/adr/`. See `docs/agents/domain.md`.
