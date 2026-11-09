# Build on Maestro at one exact revision

Issue: [#74](https://github.com/Orchestration-Maestro/maestro-packages/issues/74).
Part of [Orchestration-Maestro/maestro#4035](https://github.com/Orchestration-Maestro/maestro/issues/4035).
Contract: [Examples repository](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/specs/maestro-port.md#examples-repository).

## Contract

This repository holds community extensions and Maestro's examples, in every
language. Before the examples arrive, the workspace builds on Maestro at one
known commit and prepares its tests with Maestro's development tool. This
change adds no example and no tooling crate.

## Pin

- The root `Cargo.toml` declares every Maestro git dependency once, with one
  exact `rev`: a full commit hash on Maestro's `main`. Member crates inherit
  the entry with `workspace = true`.
- `Cargo.lock` resolves every Maestro package to that commit.
- `.github/ci.toml` installs the development tool at the same commit.

## Shared versions

Every registry package that both this lock and Maestro's lock contain resolves,
in this lock, to a version Maestro's lock also holds. A direct exact pin that
Maestro also declares uses Maestro's version: `tokio` is `=1.53.3`. Two exact
pins of one package cannot resolve together once an example crate depends on
both workspaces.

## Deferred to the first host consumer

Maestro's component-compile profile overrides (`cranelift-codegen` and
`regalloc2` at `opt-level = 3`) and the `https://github.com/bytecodealliance/wasmtime`
entry in `git_sources` name packages this lock does not hold yet. Cargo warns
about a profile override that matches no package, and cargo-deny warns about an
allowed source the lock never uses. The extension example change adds both
with the first dependency that locks the host.

## Test preparation

`.github/ci.toml` declares the preparation with the keys of the shared CI the
organisation pins:

- `test_prepare` installs `development` from `maestro-tooling` at the pinned
  commit into `target/development`, then runs
  `development prepare-test-setup prepare "$TEST_PREPARE_OUTPUT" "${CARGO_TARGET_DIR:-target/test-prepare}"`.
- `test_prepare_inputs` lists `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`,
  `.config/nextest.toml`, `examples/**` and `crates/**`.
- `test_prepare_setups` lists the nextest setups whose tests read the bundle.
  Shared CI refuses a listed setup that no `[[profile.default.scripts]]` rule
  selects, so each example change adds a setup name together with the filter
  rule that selects its tests. The list starts empty, and preparation runs in
  CI from the first change that lists a setup.

## Nextest

`.config/nextest.toml` defines the setups `author-component`, `cargo-checks`,
`native-fixture` and `executable`. Each runs
`target/test-setup/development nextest-setup <name>`. The example changes add
the filter rules.

## Local recipe

The pinned tool stops when no member crate holds the generated example region,
so a local recipe would make `just test` fail. The extension example change
adds `just prepare-tests`, which runs the same two steps with
`target/test-setup` as the output, together with the component crate that
holds the region, and makes `just test` depend on it. `just test` runs as
before until then.

## Pin moves

One change moves every Maestro `rev`, the install `rev` in `.github/ci.toml`,
the lock, and every shared version Maestro moved, and fixes what the move
breaks.

## Existing crates at the new pin

The pin moves `maestro-extensions-wasm` past four contract changes, and the
existing crates follow them:

- Event handlers return `HookFuture`, whose error is `DiagnosticErrorInfo`.
  The `maestro-prompt-suffix` input handler returns it.

- `ToolDefinition` gains the optional `render_call` and `render_result`
  renderers. The security tools set neither.
- Tool result details are `JsonText`. The security tools convert their JSON
  text into it.
- `ExtensionHost` gains further required methods. The stand-in host in the
  security extension tests refuses each one it never reaches with one message.
  Two of them return `IndexMap`, so the tests take `indexmap =2.14.2`,
  Maestro's exact pin, as a development dependency. The stand-in moves to
  Maestro's own test host once a pinned Maestro commit ships one.

## Verification

- `Cargo.lock` names one Maestro commit, equal to every `rev` and to the
  install revision.
- Every registry package both locks contain has a version Maestro's lock holds.
- The `test_prepare` steps install `development` at the pinned commit. The
  pinned tool then stops because no member crate holds the generated example
  region yet. Preparation completes once the extension example crate adds that
  region, or once the pin moves to a Maestro commit whose tool skips the
  component steps when no member holds it.
- `just test` runs the existing suites.
- Shared CI is green: the existing crates build and test, the declaration
  validates and cargo-deny passes with no stale exception.

## Scope

`Cargo.toml`, `Cargo.lock`, the two crate manifests that name
`maestro-extensions-wasm`, `crates/maestro-prompt-suffix/src/lib.rs`,
`crates/maestro-security/src/extension.rs` and its tests, `.github/ci.toml`,
`.config/nextest.toml`, `AGENTS.md`, `README.md`,
`docs/examples.md`, `docs/libraries/security-testing.md` and this file
change. No test is added; the existing suites and the preparation logs
are the proof.
