# Python: input hook and greeting tool

`hello.py` is a Python extension for the unchanged
[extension world](../README.md#world-inspected). It registers an `input` hook that
upper-cases the text and a `hello` tool that answers `Hello, <name>!`.

It needs componentize-py 0.25.1 with a one-hunk patch. The released compiler
builds the component, but the host stops it at its first callback (see
[the defect](#the-defect)). Tracking: [#57](https://github.com/Orchestration-Maestro/maestro-packages/issues/57).

| Tool | Version |
| --- | --- |
| componentize-py | 0.25.1 plus [the patch](toolchain/release-borrows-before-task-return.patch) |
| Rust | stable 1.99.0 with the `wasm32-wasip2` target |
| WASI SDK | 34.0 (`clang version 23.1.0-wasi-sdk`) |
| Maestro | `c9db49dc50b938c8804c9af29f21d7780691c49d` (world unchanged since `d3f7aa6`) |

## The defect

In componentize-py 0.25.1,
[`runtime/src/lib.rs:500-502`](https://github.com/bytecodealliance/componentize-py/blob/v0.25.1/runtime/src/lib.rs#L500-L502)
calls `func.call_task_return(&mut call)` and only then `release_borrows(...)`. Every
export except `start` receives `borrow<callback>`, so the first call traps:

```text
borrow handles still remain at the end of the call
```

The toolchain is wrong, not the world or the host
(upstream issue: [componentize-py#255](https://github.com/bytecodealliance/componentize-py/issues/255)):

- The component model traps `task.return` while the task still holds a borrow
  (`Task.return_` in the reference `definitions.py`: `trap_if(self.num_borrows > 0)`).
- Maestro's Wasmtime
  ([`ffff2e2d`](https://github.com/bytecodealliance/wasmtime/tree/ffff2e2db24a291f02429f123aba1631fa8eea67))
  checks it in `concurrent.rs` `task_complete` (line 3964), which calls
  `validate_scope_exit` and fails at `vm/component/resources.rs:402`. Wasmtime added
  that check in 43.0.0 (pull request 12550).
- `main` of componentize-py (`704719ea`) keeps the same order.

The patch moves `release_borrows` before `call_task_return`. Nothing else changes.

## Build the patched compiler

The published crate ships the prebuilt CPython library, so only the runtime is
rebuilt. Run this from the root of this repository; `$MAESTRO` is a Maestro checkout
at the revision above.

```sh
REPO=$PWD
MAESTRO=$PWD/../maestro
WORK=$REPO/../componentize-py-patched
mkdir -p "$WORK" && cd "$WORK"

curl -sSLO https://static.crates.io/crates/componentize-py/componentize-py-0.25.1.crate
curl -sSLo runtime-v0.25.1.tar.gz \
  https://github.com/bytecodealliance/componentize-py/archive/refs/tags/v0.25.1.tar.gz
curl -sSLO https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-34/wasi-sdk-34.0-x86_64-linux.tar.gz
sha256sum -c <<'EOF'
addb8010b8a05a736a7efaee36975e616bdc4762c458a3ad553405354b90ed47  componentize-py-0.25.1.crate
07351b4879b568c5fcbcb6771c2863f1742cae3d2dab0986d68a95ec145d01da  runtime-v0.25.1.tar.gz
b761e3a0721dbae9c09a0059e5fdb2bf917d1b4a8a7b430fb3b5aafb0984b2c4  wasi-sdk-34.0-x86_64-linux.tar.gz
EOF

mkdir crate runtime-src wasi-sdk pylib
tar xzf componentize-py-0.25.1.crate -C crate --strip-components=1
tar xzf runtime-v0.25.1.tar.gz -C runtime-src --strip-components=1
tar xzf wasi-sdk-34.0-x86_64-linux.tar.gz -C wasi-sdk --strip-components=1
patch -d runtime-src -p1 < "$REPO/examples/python/toolchain/release-borrows-before-task-return.patch"

zstd -d crate/libpython3.14.so.zst -o pylib/libpython3.14.so
{ cat runtime-src/pyo3-config.txt; echo "lib_dir=$WORK/pylib"; } > pyo3-config.txt

# 1. The patched async runtime, linked the way the compiler's own build does.
(cd runtime-src/runtime && \
  RUSTFLAGS="--cfg pyo3_disable_reference_pool -Clink-args=-Wl,--skip-wit-component -Clink-args=-shared -Clink-args=-L$WORK/pylib -Clink-args=-lpython3.14 -Clink-self-contained=n" \
  CARGO_TARGET_WASM32_WASIP2_LINKER="$WORK/wasi-sdk/bin/clang" \
  CARGO_TARGET_DIR="$WORK/runtime-target" \
  PYO3_CONFIG_FILE="$WORK/pyo3-config.txt" \
  cargo build --locked --release --target=wasm32-wasip2 --features=async)

# 2. The compiler, with only the async runtime replaced.
zstd -19 -f runtime-target/wasm32-wasip2/release/componentize_py_runtime.wasm \
  -o crate/libcomponentize_py_runtime_async.so.zst
(cd crate && CARGO_TARGET_DIR="$WORK/compiler-target" \
  cargo build --locked --release --bin componentize-py)
```

On 2026-10-30, with three build jobs on an 8-core Linux x86-64 machine, step 1
took 21 seconds and step 2 took 3 minutes 11 seconds. The compiler is
`$WORK/compiler-target/release/componentize-py`. The patched runtime,
`runtime-target/wasm32-wasip2/release/componentize_py_runtime.wasm`, has SHA-256
`054ce436596d64081111af7bf37ca4eac6db9373301e553100c166b6292c155a`; two separate
builds produced the same bytes.

## Build and run the extension

The `maestro-python-example` test builds the extension with the compiler on `PATH`,
installs it in a Maestro executable and checks the result against a local model. It
reads the extension world from `target/wit` and the executable from `target/maestro`,
and the executable reads its themes from `theme/` beside it, so the test is ignored
unless asked for. `$MAESTRO` is a Maestro checkout at the revision above:

```sh
cd "$REPO"
mkdir -p target
cp -r "$MAESTRO/crates/maestro-extensions-wasm/wit" target/wit
cp "$MAESTRO/target/release/maestro" target/maestro
cp -r "$MAESTRO/crates/maestro-theme/assets/theme" target/theme
PATH="$WORK/compiler-target/release:$PATH" \
  cargo nextest run -p maestro-python-example --run-ignored only
```

Build the executable with `cargo build --release --locked -p maestro` in `$MAESTRO`.
The one used here has SHA-256
`fd529e5646f03c33b487763ed681fd26fa864dcc2be0f1d9b909c8f0ad3c0726`.

The test compiles the extension with this command:

```sh
componentize-py -d target/wit -w extension componentize -p examples/python hello \
  -o hello.wasm
```

It then clears live credentials, creates a temporary project and registers the
component there with this install command:

```sh
maestro install hello.wasm -l
```

It submits the prompt `greet Ada` through the real host. A provider on an ephemeral
loopback port first answers with a call to `hello`, then with a final reply. No external
model is contacted, and the registration is removed with the project. Do not install
this extension into a real project.

Observed on 2026-10-30:

```text
PASS [  14.600s] (1/1) maestro-python-example::python_example the_host_runs_the_component
Summary [  14.601s] 1 test run: 1 passed, 0 skipped
```

The test asserts that the first model request ends with `GREET ADA`, which shows the
hook's output reaching the model. It asserts that the model is offered `hello` and that
its second request carries the tool result `Hello, Ada!`.

## Red run: the released compiler

Run the same test with the released compiler first on `PATH`:

```sh
uv tool install componentize-py==0.25.1
cargo nextest run -p maestro-python-example --run-ignored only
```

The test fails on its exit-status assertion. The host rejects the component at
`task.return invoke-event`:

```text
panicked at crates/maestro-python-example/tests/python_example.rs:62:5:
...
   19: ... - libpython3.14.so!_PyFunction_Vectorcall: borrow handles still remain at the end of the call
```

A compiler rebuilt from the same sources without the patch fails the same way, so the
patch is the only difference between the two runs.

## Notes for authors

- Keep state at module level. componentize-py 0.25.1 builds a separate `Guest` object
  for each exported function, so a table stored on `self` in `start` is empty in
  `invoke_event`. `main` shares one object per interface.
- `register_tool` returns an import route. `hello.py` reads one step; its tool has no
  executable schema nodes, so that step is `Done`. A tool with such nodes also receives
  `Invoke` steps.
- Every export the example does not serve fails with its declared error type
  (`reject_unregistered`), which keeps the generated async dispatch intact.
- Generated bindings are build outputs, not source. Pyright reads
  `target/guest-python`, which `componentize-py ... bindings target/guest-python`
  writes.
