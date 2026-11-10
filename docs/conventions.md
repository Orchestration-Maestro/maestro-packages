# Repository checks

This repository tracks Rust and nothing else. Two checks enforce that, and one
hook formats and lints what you commit.

## Quick start

Run both checks on the whole workspace:

```sh
cargo test -p maestro-packages-conventions --locked
```

`just check` runs the workspace check on every run, and shared CI runs it with
the rest of the suite.

## What the checks require

`maestro-packages-conventions` exports `check_workspace(root)`. It runs both checks
and returns every failure at once as a list of `Violation` values, so a caller
matches on the kind of failure and never on message text.

| Check | Fails when | Fix |
|---|---|---|
| Workspace lints | A member in `[workspace] members` has no `[lints] workspace = true` | Add the table to the member's `Cargo.toml` |
| Tracked sources | The git index holds a JavaScript, TypeScript, Python, shell or PowerShell file (see the extensions below) | Remove the file, or write it in Rust |

The tracked-sources check rejects these extensions, in any letter case:

| Language | Extensions |
|---|---|
| JavaScript | `.js`, `.mjs`, `.cjs`, `.jsx` |
| TypeScript | `.ts`, `.mts`, `.cts`, `.tsx` |
| Python | `.py`, `.pyi`, `.pyw` |
| Shell | `.sh`, `.bash`, `.zsh`, `.ps1` |

Member patterns are a path or a directory followed by `/*`. Any other pattern stops
the check, so a new glob form is a decision and not a silent gap.

The tracked-sources check reads the index of the repository at the root you pass,
so a file you staged counts and a file git ignores does not. Logic, setup, tooling,
tests, examples and test child processes are all Rust.

## The commit hook

`just pre-commit` installs Maestro's `development` tool from `maestro-tooling` at
the pinned Maestro commit into `target/development`, then runs
`development pre-commit`. The tool captures the staged paths, formats the
crates that own them, stages only the captured paths that survive formatting,
and runs Clippy with warnings denied on those crates. Shared CI runs the
dependents and the workspace-wide checks.

The install commit is the same commit as every `rev` in the root `Cargo.toml`
and in `.github/ci.toml`. [Move the pin](examples.md#move-the-pin) lists all
three places.
