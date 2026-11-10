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

Run one family's tests after a preparation. `TEST_PREPARE_OUTPUT` names the
directory the preparation staged, which the `executable` setup reads; `just
test` sets it:

```sh
just prepare-tests
TEST_PREPARE_OUTPUT="$PWD/target/test-setup" cargo nextest run --locked -p maestro-extension-examples
```

## Layout

The `examples/` directory holds every example source once. The family crates
point into it by relative path, and Maestro publishes the directory as it is.

```text
examples/
  README.md              index of every folder
  extensions/            extension sources, each with a usage page
  sdk/                   SDK examples, the interactive RPC client demo
  rpc_extension_ui.rs    terminal RPC client
  web/                   browser chat example
```

`extensions/`, `sdk/`, `rpc_extension_ui.rs` and `web/` are all here, each with its family crate.

## Crates

| Crate | Builds and verifies |
|---|---|
| `maestro-extension-examples` | extension examples, as WebAssembly components |
| `maestro-sdk-examples` | SDK examples, the terminal RPC client and the RPC client demo |
| `maestro-web-example` | the browser chat example |

## Browser example

`maestro-web-example` builds the browser chat in `examples/web/` for
`wasm32-unknown-unknown` and runs its browser tests in Chromium. See
[the example's page](../examples/web/README.md) for what it shows.

Build and serve the page on loopback:

```sh
mise exec -- just browser-chat-example --port 9000
```

`just browser-build` builds and binds the page without serving it, and
`just browser-chat-serve` serves what it built. Each recipe reads Cargo's
`CARGO_TARGET_DIR` when it is set, and `target` otherwise.

Run the browser tests with a Chromium executable:

```sh
CHROMIUM_PATH=/path/to/chrome mise exec -- just browser-test
```

`just browser-test` runs the ignored `browser::` tests of the crate through
nextest. The `browser-exports` setup in `.config/nextest.toml` runs once before
them: it compiles `crates/maestro-web-example/tests/program` for
`wasm32-unknown-unknown` with `--cfg test`, requires the `wasm-bindgen` command line
to match the locked library, binds the program into `target/browser-exports` and
exports that directory as `MAESTRO_BROWSER_EXPORTS`. Each test then runs one export
in its own browser through Maestro's driver, which serves the package on loopback
and checks that the package is unchanged afterwards.

Shared CI runs `just browser-test` in the pinned Chromium job because
`browser_test_crates` in `.github/ci.toml` names the crate. Pull requests skip that
job. Merge groups run it when the crate or one of its dependencies changed, and
pushes to main always run it. `browser_build` in `.github/ci.toml` makes every run
execute `just browser-build`, which builds the `chat` example for
`wasm32-unknown-unknown` and binds it. A startup that no longer compiles fails the run.

The program holds the exports and only the browser helpers they read. It reaches
`maestro-web` through its public API and reads the brand pack with
`BrandPack::shipped()`; the repository carries no copy of the pack. A helper that
needs a private item exposes a gap in that API; report the gap.

## SDK and RPC examples

Run an in-process SDK sample:

```sh
cargo run -p maestro-sdk-examples --example minimal
```

Run every SDK and RPC check:

```sh
just prepare-tests
TEST_PREPARE_OUTPUT="$PWD/target/test-setup" cargo nextest run --locked -p maestro-sdk-examples
```

`maestro-sdk-examples` builds three groups of sources from `examples/` and runs
their tests:

| Source | Example target | Checks |
|---|---|---|
| `sdk/*.rs`, with `sdk/support/` and `sdk/tests/` | one per sample, such as `minimal` | each sample's own tests, run in-process |
| `rpc_extension_ui.rs`, with `sdk/rpc_extension_ui/` | `rpc_extension_ui` | the terminal client's tests |
| `sdk/rpc_client_demo/` | `rpc_client_demo` | the demo's checks, in the crate's `tests/integration.rs` |

The thirteen SDK samples use Maestro's scripted model provider and disposable
configuration, so they need no credentials and make no paid request. Their tests
need no preparation.

The RPC checks start Maestro's native test peers instead of a real Maestro
child. They cover correlated dialogs and literal replies, terminal start and
stop, the termination marker, exit outcomes, LF and CRLF framing with U+2028
and U+2029, refusals, early end of stream and cleanup. The `native-fixture`
setup exports `MAESTRO_NATIVE_FIXTURE` for the `rpc_extension_ui` example
binary and the crate's integration binary. The interactive demo's checks run
on Unix.

Add a sample in three steps:

1. Put the source in `examples/sdk/`, with its tests in a trailing `tests`
   module.
2. Add an `[[example]]` entry with `test = true` and a relative `path` to
   `crates/maestro-sdk-examples/Cargo.toml`.
3. Add its row to `examples/sdk/README.md`.

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

The crate's one integration program, `tests/integration.rs`, holds these test
modules:

| Module | Loads the examples through |
|---|---|
| `guest` | Maestro's public test harness: the production engine over in-memory history, credentials and catalog, and the scripted model provider |
| `host` | Maestro's native host: the artifact factory, the extension runner and the toolkit widgets |
| `executable` | Maestro's executable over RPC, and its SDK session in process |
| `shipped_examples` | the folder indexes: every entry is listed, and every listed path exists |

A `guest` test names the built component by its setup variable, loads it with
`Harness::load`, and reaches the extension only through what the application
calls: the runner's registered tools and commands, its events and the terminal
UI the test installs. Nothing in it fakes the extension host. The recorded
corpora sit beside the tests as `<name>_expected.json`, byte for byte as
recorded.

The `guest` tests share one set of helpers in `tests/guest/support/`: the
recording terminal UI, the callback errors a runner reports instead of
returning them, the paused-clock runtimes and the rows a renderer draws. A
test that waits on the extension waits on a signal a helper gives, such as the
next redraw or the next recorded request. It never sleeps or polls.

Some behaviour has no form on the public harness yet: native effects, injected
clocks and random sources, a signal for the moment a guest arms its next timer
wait, provider transport, and failures injected at a chosen registration. Those
cases have no `guest` test, and
[Orchestration-Maestro/maestro#4448](https://github.com/Orchestration-Maestro/maestro/issues/4448)
lists each missing seam with the examples that wait on it.

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
4. Add the test as `<name>_example.rs` under `tests/guest/`, `tests/host/` or
   `tests/executable/`, and declare it in that folder's `mod.rs`, in name
   order. The test reads its component from the variable; the
   `author-component` setup exports it. Start a `guest` test from
   `tests/guest/hello_example.rs`.
5. Run `just prepare-tests`, then
   `TEST_PREPARE_OUTPUT="$PWD/target/test-setup" cargo nextest run --locked -p maestro-extension-examples`.

An example needs no setup filter of its own. The nextest rules select the
whole crate for the `author-component` and `native-fixture` setups, the
`executable::` tests for the `executable` setup, and `host::extension_guide::`
for the `cargo-checks` setup.
