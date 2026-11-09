# Maestro's examples

This repository holds Maestro's examples, in every language, beside its
community extensions. Each example family has one crate that builds the
sources and runs their tests. The tests build on Maestro at one pinned commit
and use the components, executable and native test child that Maestro's
development tool prepares.

## Quick start

Run the tests:

```sh
just test
```

The first example family adds `just prepare-tests`, and `just test` then runs
it first. From then on, the first preparation compiles Maestro's development
tool and the programs the tests start. Later runs reuse the installed tool
while the pinned commit stays the same.

## Layout

The `examples/` directory holds every example source once. The family crates
point into it by relative path, and Maestro publishes the directory as it is.

```text
examples/
  README.md              index of every folder
  extensions/            extension sources, each with a usage page
  sdk/                   SDK examples, the Python RPC client, the interactive RPC client demo
  rpc_extension_ui.rs    terminal RPC client
  web/                   browser chat example
  python/  typescript/   extensions in other languages
```

`python/` and `typescript/` are here today. The other folders arrive with their
family crates.

## Crates

| Crate | Builds and verifies |
|---|---|
| `maestro-extension-examples` | extension examples, as WebAssembly components |
| `maestro-sdk-examples` | SDK examples, the terminal RPC client and the RPC client demo |
| `maestro-web-example` | the browser chat example |
| `maestro-python-example`, `maestro-typescript-example` | the other-language extensions |

The first three crates arrive with their family crates. Only the two
other-language crates exist today.

## The Maestro pin

The root `Cargo.toml` declares every Maestro crate once, as a git dependency
with one exact `rev`. Member crates inherit the entry:

```toml
maestro-extensions-wasm.workspace = true
```

Every registry package that this workspace and Maestro both lock uses a
version Maestro's lock holds, and exact pins Maestro also declares, such as
`tokio`, use Maestro's version. Two exact pins of one package cannot resolve
together.

### Move the pin

Move the pin in one change:

1. Set the new commit as the `rev` of every Maestro entry in the root
   `Cargo.toml`.
2. Set the same commit as the install `--rev` in `test_prepare` in
   `.github/ci.toml`, and in the `prepare-tests` recipe in `justfile` once
   it exists.
3. Update the lock with `cargo update -p maestro-test-directory`, then align
   every shared package with Maestro's lock at that commit, using
   `cargo update -p <package>@<version> --precise <version>`.
4. Match every exact pin Maestro moved, and update its line in
   `docs/libraries/`.
5. Fix what the move breaks, then run `just check` and `just test`.

Check the result:

```sh
rg -n 'git = "https://github.com/Orchestration-Maestro/maestro"' -g Cargo.toml
rg -o 'Orchestration-Maestro/maestro\?rev=[0-9a-f]+#[0-9a-f]+' Cargo.lock | sort -u
```

Every hit of the first search carries the `rev`, and the second search shows
one commit.

## Test preparation

Preparation runs two steps:

1. `cargo install` builds `development` from `maestro-tooling` at the pinned
   commit into `target/development`.
2. `development prepare-test-setup prepare target/test-setup <target>` builds
   the example components and installs `maestro` and `native_fixture` from the
   locked Maestro commit into the bundle at `target/test-setup`. `<target>` is
   `CARGO_TARGET_DIR`, or `target/test-prepare` when that is unset. The
   component build waits for the first example family's component crate.

Shared CI runs them from `test_prepare` in `.github/ci.toml` once per run when
a selected test reads the bundle, then hands the bundle to every test shard.
Locally, the `just prepare-tests` recipe runs them with `target/test-setup` as
the bundle. The recipe arrives with the first example family, whose component
crate the preparation needs.

The nextest setups in `.config/nextest.toml` run
`target/test-setup/development nextest-setup <name>` and export what the tests
read:

| Setup | Exports |
|---|---|
| `author-component` | `MAESTRO_<NAME>_COMPONENT` for each example component |
| `cargo-checks` | `MAESTRO_GUIDE_COMPONENT` |
| `native-fixture` | `MAESTRO_NATIVE_FIXTURE` and `MAESTRO_CHILD_TOOLCHAIN_PROBE` |
| `executable` | `MAESTRO_EXECUTABLE` |

A setup runs only for the tests a `[[profile.default.scripts]]` filter rule
selects. Each family crate adds the rules for its tests, and lists the same
setup names in `test_prepare_setups` in `.github/ci.toml`; shared CI refuses a
listed setup that no rule selects.
