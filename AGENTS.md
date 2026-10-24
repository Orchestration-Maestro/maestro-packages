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
