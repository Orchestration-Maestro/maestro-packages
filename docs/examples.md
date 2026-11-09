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

`just test` runs `just prepare-tests` first. The first preparation compiles
Maestro's development tool, the example components and the programs the tests
start. Later runs reuse the installed tool while the pinned commit stays the
same.

Run one family's tests after a preparation:

```sh
just prepare-tests
cargo nextest run --locked -p maestro-extension-examples
```

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

`extensions/`, `python/` and `typescript/` are here today. The other folders
arrive with their family crates.

## Crates

| Crate | Builds and verifies |
|---|---|
| `maestro-extension-examples` | extension examples, as WebAssembly components |
| `maestro-sdk-examples` | SDK examples, the terminal RPC client and the RPC client demo |
| `maestro-web-example` | the browser chat example |
| `maestro-python-example`, `maestro-typescript-example` | the other-language extensions |

`maestro-extension-examples` and the two other-language crates exist today.
The SDK and web crates arrive with their family crates.

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
   `.github/ci.toml`, and in the `development` recipe in `justfile`.
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
   components come from the component crate, `maestro-extension-examples`.

Shared CI runs them from `test_prepare` in `.github/ci.toml` once per run when
a selected test reads the bundle, then hands the bundle to every test shard.
Locally, the `just prepare-tests` recipe runs them with `target/test-setup` as
the bundle.

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

## Extension examples

`examples/extensions/` holds one source file or one directory per example,
each with a same-name Markdown usage page, and its `README.md` indexes them.
The `maestro-extension-examples` crate builds each example as a WebAssembly
component and loads it in Maestro's host and executable.

The crate's manifest holds one generated `cdylib` target per example, between
`# example-index:start` and `# example-index:end`. `just example-index`
renders that region, the index table in `examples/extensions/README.md` and
the guide fixture's lock from the sources. `just example-index-check` checks
all three without rewriting them, and `just check` runs it.

The crate's one integration program, `tests/integration.rs`, holds three test
modules:

| Module | Loads the examples through |
|---|---|
| `host` | Maestro's native host: the artifact factory, the extension runner and the toolkit widgets |
| `executable` | Maestro's executable over RPC, and its SDK session in process |
| `shipped_examples` | the folder indexes: every entry is listed, and every listed path exists |

### Add an extension example

1. Add `examples/extensions/<name>.rs` with an opening `//!` description, and
   a same-name Markdown usage page. A larger example is a directory with a
   `main.rs` entry point; its target name is the directory name with hyphens
   replaced by underscores.
2. If a test loads the example, add this line after its opening docs:
   `// maestro-component: MAESTRO_<NAME>_COMPONENT`. Preparation builds every
   marked example and exports its path under that variable. Keep an existing
   variable's spelling unchanged.
3. Run `just example-index` to render the target and the index row.
4. Add the test as `<name>_example.rs` under `tests/host/` or
   `tests/executable/`, and declare it in that folder's `mod.rs`, in name
   order. The test reads its component from the variable; the
   `author-component` setup exports it.
5. Run `just prepare-tests`, then
   `cargo nextest run --locked -p maestro-extension-examples`.

An example needs no setup filter of its own. The nextest rules select the
whole crate for the `author-component` and `native-fixture` setups, the
`executable::` tests for the `executable` setup, and `host::extension_guide::`
for the `cargo-checks` setup.
