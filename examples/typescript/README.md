# TypeScript: a working extension

**Experimental toolchain.** `jco componentize` with its QuickJS backend, pinned in
`mise.toml`: `@bytecodealliance/jco` **1.40.0** and Node.js **24.20.0**.
`jco guest-types --help` labels guest types experimental.

`extension.ts` registers an `input` hook that appends an instruction to each prompt you
type, and a `hello` tool that greets the name the model passes. It keeps its closures in a
table keyed by the id of the host-owned `callback` resource, as the Rust guest does, and
drops an entry when the host calls `release`. It exports only the guest functions this
extension needs: `start`, `release`, `invokeEvent` and `invokeTool`.

## Build

Use the complete [extension world](../README.md#world-inspected), unchanged, from a sibling
Maestro checkout. From this repository:

```sh
mise install
mkdir -p target
cp -r ../maestro/crates/maestro-extensions-wasm/wit target/wit
cd examples/typescript
mise exec -- jco componentize extension.ts --backend quickjs \
  --wit ../../target/wit --world-name extension --out ../../target/extension.wasm
```

Observed on 2026-10-29: exit 0 in 1 second, `OK Successfully written
../../target/extension.wasm.` (2 MB). The bundler also prints an `UNRESOLVED_IMPORT` warning for
`maestro:extension/host@0.1.0`. It cannot see the world; `jco` supplies that import from
the WIT.

`jco guest-types ../../target/wit --world-name extension --out-dir ../../target/types`
writes the declarations that the example imports. With TypeScript 7.0.2, the compiler jco
1.40.0 bundles, `tsc --strict --noEmit --target es2022 --lib esnext --module esnext
--moduleResolution bundler ../../target/types/extension.d.ts extension.ts` exits 0.

## Install and run

Install into a throwaway project, because the hook rewrites every prompt typed there:

```sh
cd "$(mktemp -d)"
maestro install /path/to/maestro-packages/target/extension.wasm -l
maestro
```

Ask the model to greet someone. It is offered the `hello` tool, and the prompt it receives
ends with `Say: Frank is the best.`

From the repository root, the `maestro-typescript-example` tests run the same steps against
a local model. They need the world and a Maestro executable in `target/`, so they are
ignored unless asked for:

```sh
cp /path/to/maestro target/maestro
mise exec -- cargo nextest run -p maestro-typescript-example --run-ignored only
```

Observed on 2026-10-29: 2 tests run, 2 passed. The host test installs the component, sends
`greet Ada` and checks that the model received `greet Ada` plus the suffix and the `hello`
tool, then received the tool result `Hello, Ada!` after calling it.

The executable was built from Maestro `c9db49dc50b938c8804c9af29f21d7780691c49d` with
`cargo build --release --locked -p maestro` on the remote build host (receipt
`0ab143db87d7-1793325906262976707`, digest
`c9d16d2c4155608ff1c50fa1db7831d723153d330c7fcb2b1b67e660d235caf5`). Its SHA-256 is
`715ed7b1cc9365f579577cd69a37032368eecddd3db8ed4a4e162bde76baee7a`. The `wit/`
directory at that revision is byte-identical to the one at the revision the world section
names.

## Why the QuickJS backend

jco 1.39.0 failed both ways on this world, for reasons outside it:

- Its QuickJS backend (componentize-qjs 0.4.5) snapshots the guest with Wasmtime 47.
  Wasmtime 47 `component/linker.rs:390`, `define_unknown_imports_as_traps`, stubs every
  import with a synchronous function, so any `async func` import fails with `type mismatch
  with async`. Wasmtime 48 fixed it; jco 1.40.0 carries componentize-qjs 0.5.0 on Wasmtime
  50.
- Its default backend (ComponentizeJS 0.23.0) panics at `bindgen.rs:721:48`. That line in
  `spidermonkey-embedding-splicer` is `FunctionKind::AsyncFreestanding => todo!()`: the
  backend implements no async imports. No release changes this, and its development branch
  still has the `todo!()`.

Without `--backend quickjs`, jco 1.40.0 still hits the second failure, so keep the flag.
