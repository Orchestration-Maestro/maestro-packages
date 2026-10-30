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
maestro list
maestro
```

Observed on 2026-10-29, in an empty directory with an isolated home (paths shortened):
`install` exits 0 and prints

```text
Installing /path/to/maestro-packages/target/extension.wasm...
Installed /path/to/maestro-packages/target/extension.wasm
```

and `list` exits 0 and prints

```text
Project packages:
  ../../../path/to/maestro-packages/target/extension.wasm
    /path/to/maestro-packages/target/extension.wasm
```

Plain `maestro` opens the interactive terminal and needs a live model, so it is not run here.
Ask the model to greet someone: it is offered the `hello` tool, and the prompt it
receives ends with `Say: Frank is the best.` The tests below observe that against a local
model.

From the repository root, the `maestro-typescript-example` tests run the same steps against
a local model. They need the world and a Maestro executable in `target/`, and the executable
reads its themes from `theme/` beside it when it was not built in place, so they are ignored
unless asked for:

```sh
cp /path/to/maestro target/maestro
cp -r /path/to/maestro/crates/maestro-theme/assets/theme target/theme
mise exec -- cargo nextest run -p maestro-typescript-example --run-ignored only
```

Observed on 2026-10-29: 3 tests run, 3 passed:

```text
        PASS [   2.972s] the_pinned_jco_builds_the_unchanged_world
        PASS [   5.586s] the_host_runs_the_component
        PASS [   8.880s] input_from_another_extension_reaches_the_model_unchanged
     Summary [   8.881s] 3 tests run: 3 passed, 0 skipped
```

The host test installs the component, sends `greet Ada` and checks that the model received
`greet Ada` plus the suffix and the `hello` tool, then received the tool result `Hello,
Ada!` after calling it. The third test installs the example next to a probe extension (a
test fixture) that queues a user message from a tool, and checks that the model receives
that message exactly as sent: the example's hook leaves input from other extensions alone.
The `release` function has no test, because the host never calls a released handle again
and nothing outside the guest can see the call.

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
