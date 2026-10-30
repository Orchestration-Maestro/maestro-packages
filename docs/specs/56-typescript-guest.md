# TypeScript extensions build and run

Issue: [#56](https://github.com/Orchestration-Maestro/maestro-packages/issues/56).
Follows [#41](https://github.com/Orchestration-Maestro/maestro-packages/issues/41), which
closed with a blocker report for the TypeScript toolchain.

## Contract

A TypeScript guest compiles against `maestro:extension/extension@0.1.0` as it stands in
Maestro: the complete `wit/` directory, unchanged, async functions and borrowed callback
parameters included. The pinned toolchain produces a component that Maestro's host installs
and runs. The example registers one event hook and one tool, and keeps their callbacks
until the host releases them.

## Cause of the #41 failures

Neither failure comes from the world.

| Backend | Failure | Root | Status |
| --- | --- | --- | --- |
| QuickJS | `[method]file-mutation.wait` has the wrong type: type mismatch with async | Wasmtime 47 `component/linker.rs:390`: `define_unknown_imports_as_traps` stubs every import with a synchronous function, so an `async func` import fails the type check. componentize-qjs 0.4.5 (jco 1.39.0) calls it while snapshotting the guest. | Fixed in Wasmtime 48 (`7e8021c83`). jco 1.40.0 carries componentize-qjs 0.5.0 on Wasmtime 50. |
| Default (StarlingMonkey) | panic at `bindgen.rs:721:48` | ComponentizeJS 0.23.0 `spidermonkey-embedding-splicer/src/bindgen.rs:721`: `FunctionKind::AsyncFreestanding => todo!()`. The async method and static arms next to it, and the async ABI variants, are `todo!()` too. | Still `todo!()` on the development branch. No release fixes it. |

The world is right as it stands. Native async imports and exports are the world's design,
and changing them would only route around a missing toolchain feature.

## Decisions

- Build with the QuickJS backend of jco 1.40.0. The default backend cannot build any world
  that imports an async function; the README says so and names the line.
- Pin Node.js 24.20.0 and jco 1.40.0 in `mise.toml`, which the shared CI already installs
  for tests (`test_tools = true`). The version of jco is the whole fix.
- The example stays one TypeScript file with the callback table the Rust guest keeps:
  closures keyed by the identity of the host-owned `callback` resource, dropped on
  `release`.
- Remove the diagnostic source and the TypeScript blocker text. Python stays a blocker
  report.

## Verification

- A build test runs the pinned jco on the unchanged world and checks that the component
  imports `maestro:extension/host@0.1.0` and exports `maestro:extension/guest@0.1.0`. It
  fails with jco 1.39.0.
- A host test installs the component into a Maestro executable and checks that the hook
  rewrote the prompt, the model was offered the tool and the tool result reached the model.
- A second host test installs the example next to a probe extension that queues a user
  message from a tool, and checks that the model receives the message as sent, because
  the example's hook leaves input from other extensions alone. `release` has no test: the
  host never calls a released handle again, so nothing outside the guest observes it.
- The host steps (executable, scripted model, install) live in one module that names no
  guest language, so another guest's checks reuse it.
- All tests read the world from `target/wit` and the executable from `target/maestro`, so
  they are ignored unless asked for. Cargo's locked SDK revision carries an older world than
  the host under test, and no released executable exists yet.
- The example type-checks under `strict` against the declarations `jco guest-types`
  generates. This was checked once with the compiler that jco bundles; no build step runs it.

## Scope

The example, its documentation, the pin and the tests change. The world, the host and the
Python report stay as they are.
