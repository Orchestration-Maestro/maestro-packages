# Track only Rust: tooling and examples

Issue: [#102](https://github.com/Orchestration-Maestro/maestro-packages/issues/102).

## Contract

No `.py`, `.js`, `.ts` or `.sh` file is tracked, and a check fails when one is
added. The tooling that those files held runs in Rust, in Maestro's existing
development tooling and in one workspace member.

## Tooling

- The lint-inheritance guard becomes the `maestro-packages-conventions` crate.
  Its single job: check repository rules that cargo and Clippy do not check.
  It depends on `toml`, pinned at `=1.1.6`, the version Maestro's tooling locks.
- The same crate fails on any tracked `.py`, `.js`, `.ts` or `.sh` file. It reads
  `git ls-files`, so a staged file counts.
- `just _check` runs the crate's real-workspace test.
- The pre-commit helper becomes `development pre-commit` from `maestro-tooling` at
  the pinned Maestro commit. `just pre-commit` installs the tool into
  `target/development` and runs it.
  The hook no longer selects reverse dependents or runs rustdoc; shared CI does.

## Examples

The Python and TypeScript extensions, the TypeScript probe fixture and the two
crates that built and ran them are removed. Maestro's own Rust examples already
show each behaviour:

| Removed | Rust example |
|---|---|
| `hello` tool (Python, TypeScript) | `examples/extensions/hello.rs` |
| `input` hook (Python upper-case, TypeScript suffix) | `examples/extensions/input_transform.rs`, including input from another extension passing through |
| Probe that queues a follow-up user message | `examples/extensions/send_user_message.rs`, the `/followup` command |

The example index and the guides name no non-Rust example. The Node.js and jco
tools, the Pyright configuration and the three specs of the removed guests go
with them.

## Verification

- `real_workspace::this_workspace_inherits_lints_and_tracks_only_rust` passes.
- Each fixture test in the crate fails against a stub of `check_workspace` and
  passes against the real one.
