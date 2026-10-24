# maestro-extensions

- Issue first: every change starts with a GitHub issue in this repository.
- One crate, one job; no duplicated code across crates (owner OD-413).
- Quality bar: workspace lints are `forbid`; every public and private item is documented; 500 lines per file; 5 params; 60-line functions.
- Sign every commit; subject at most 71 characters.
- Builds on the AWS remote-build host (`maestro-remote-build run ...`), never the local Qdrant or model router.
- No secrets in messages, tests or fixtures.
