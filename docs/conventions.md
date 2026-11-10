# Repository checks

This repository tracks Rust and nothing else. Two checks hold that line, and one
hook runs the formatter and Clippy on what you commit.

## Quick start

Run both checks on the whole workspace:

```sh
cargo test -p maestro-packages-conventions --locked
```

`just check` runs the workspace check from this crate on every run, and shared CI
runs it with the rest of the suite.

## What the checks require

`maestro-packages-conventions` exports `check_workspace(root)`. It runs both checks
and reports every failure at once.

| Check | Fails when | Fix |
|---|---|---|
| Workspace lints | A member in `[workspace] members` has no `[lints] workspace = true` | Add the table to the member's `Cargo.toml` |
| Tracked sources | The git index holds a `.py`, `.js`, `.ts` or `.sh` file | Remove the file, or write it in Rust |

Member patterns are a path or a directory followed by `/*`. Any other pattern stops
the check, so a new glob form is a decision rather than a silent gap.

The tracked-sources check reads the index, so a file you staged counts and a file
git ignores does not. Extensions match without regard to case. Logic, setup, tooling,
tests, examples and test child processes are all Rust.

## The commit hook

`just pre-commit` installs Maestro's `development` tool from `maestro-tooling` at
the pinned Maestro commit into `target/development`, then runs
`development pre-commit`. The tool captures the staged paths, formats the
crates that own them, stages only the captured paths that survive formatting,
and runs Clippy with warnings denied on those crates. Dependents and the
workspace-wide checks run in shared CI.

The install commit is the same commit as every `rev` in the root `Cargo.toml`
and in `.github/ci.toml`. [Move the pin](examples.md#move-the-pin) lists all
three places.
