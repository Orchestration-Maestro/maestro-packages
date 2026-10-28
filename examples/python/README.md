# Python: async borrowed callback blocked

**Diagnostic only, not a working extension.** Toolchain:
**componentize-py 0.25.1**. It parses the complete
[extension world](../README.md#world-inspected), generates bindings and builds a
valid component. Invoking an async callback with a borrowed resource traps at
runtime, so no working Python extension is shipped.

`borrow_diagnostic.py` isolates the failure. It inherits the generated export
base, registers one `input` observer and retains its callback owner until
release. The observer returns no edits or decision and retains no invocation
operands. Other exported callbacks reject calls with their declared WIT error
types. This keeps the generated async dispatch without copying a guest SDK.

## Build the diagnostic

Install the pinned compiler on your build machine:

```sh
uv tool install componentize-py==0.25.1
```

From this repository with the sibling Maestro checkout at
`d3f7aa6a9906d23350f4b28a88a41e0fde8195a0`:

```sh
WIT="$PWD/../maestro/crates/maestro-extensions-wasm/wit"
BUILD_DIR="$PWD/target"
mkdir -p "$BUILD_DIR"
componentize-py --version
componentize-py -d "$WIT" -w extension bindings "$BUILD_DIR/guest-python"
componentize-py -d "$WIT" -w extension componentize \
  -p examples/python borrow_diagnostic -o "$BUILD_DIR/borrow-diagnostic.wasm"
wasm-tools validate --features cm-async "$BUILD_DIR/borrow-diagnostic.wasm"
```

Observed on 2026-10-28: `componentize-py 0.25.1`; binding generation,
component compilation and validation with wasm-tools 1.262.0 exit 0. Compilation
prints `Component built successfully`. Component WIT inspection preserves the
async exports and borrowed callback parameters.

Generated bindings are build outputs, not checked-in source. The repository's
Pyright configuration reads `target/guest-python` for editor checks.

## Install and run the diagnostic

Use the executable described by the [matching-host receipt](../README.md#matching-host)
as `target/maestro`. Its source revision is
`d3f7aa6a9906d23350f4b28a88a41e0fde8195a0`.
After retrieving the component into `target/borrow-diagnostic.wasm`, run:

```sh
python3 examples/python/diagnose_host.py \
  "$PWD/target/maestro" "$PWD/target/borrow-diagnostic.wasm" "$PWD/../maestro"
```

The third argument supplies the source-built host's package root, including
its theme assets. Python **3.12.3** ran this driver. It clears live credentials,
creates a temporary project and registers the diagnostic there with this
install command:

```sh
maestro install "$COMPONENT" -l
```

It submits one input through the real host using a provider on an ephemeral
loopback port. No external model is contacted, and the temporary registration
is removed with the project. Do not install this diagnostic into a real project.

Observed output, exit 0:

```text
CONFIRMED async invoke-event: borrow handles still remain at the end of the call
Host continued after rejecting the component.
```

Exit 0 means the driver confirmed the expected failure, not that the extension
worked. The host reports a component fault at `task.return invoke-event` and
continues without that component. No successful extension run is claimed.

## Exact blocker

In componentize-py **0.25.1**,
[`runtime/src/lib.rs:500-502`](https://github.com/bytecodealliance/componentize-py/blob/v0.25.1/runtime/src/lib.rs#L500-L502)
calls `func.call_task_return(&mut call)` before `release_borrows(...)`.
The runtime still holds the imported callback borrow when the canonical ABI
checks task completion, producing:

```text
borrow handles still remain at the end of the call
```

The unchanged world requires `borrow<callback>` on async `invoke-event` and
`invoke-tool`; converting them to synchronous functions would change the
contract. The diagnostic neither retains the invocation borrow nor patches
compiler internals. A toolchain fix must release these borrows before task
completion; then registration, event and tool behavior need host qualification.
