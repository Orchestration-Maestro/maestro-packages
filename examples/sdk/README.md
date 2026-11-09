# SDK examples

Use Maestro through an in-process session or a child process in RPC mode.
The [RPC integration guide](../../docs/rpc.md) describes the wire records;
the [Rust client](../../docs/client.md) provides typed subprocess operations.

| File | Purpose |
|---|---|
| `rpc.py` | Inspect session state or print one prompted run with an LF-only JSONL reader. |
| `rpc_client_demo/` | An interactive terminal client of a Maestro child in RPC mode, built on the Rust client. |
| `minimal.rs` | Prompt a session, receive text deltas and inspect stored messages. |
| `custom_model.rs` | Find catalog models, select one explicitly and request a thinking level. |
| `custom_prompt.rs` | Replace a system prompt or append instructions, then prompt each session. |
| `skills.rs` | Filter discovered skills and add an SDK-attributed skill. |
| `tools.rs` | Select read-only or custom built-in tools for a supplied working directory. |
| `extensions.rs` | Register an inline extension with a command, events and an executable tool. |
| `context_files.rs` | Discover project instructions and add caller-supplied context files. |
| `prompt_templates.rs` | Merge discovered templates with a deployment template and expand it. |
| `api_keys_and_oauth.rs` | Supply credential owners, custom paths and non-persisted runtime keys. |
| `settings.rs` | Apply runtime preferences, change a persistent preference and flush it. |
| `sessions.rs` | Use memory history, create a file, continue it, list sessions and reopen one. |
| `full_control.rs` | Select model, thinking, settings, tools and resources without discovery. |
| `session_runtime.rs` | Create directory-bound services, replace the session and rebind subscriptions. |

## Inspect a session

From the repository root, with Python 3 and Maestro on `PATH`:

```sh
python3 examples/sdk/rpc.py -- maestro --mode rpc --no-session --no-tools --offline --no-extensions --no-skills --no-prompt-templates --no-context-files
```

This sends `get_state` without a model request. The example prints protocol
records to stdout and leaves child diagnostics on stderr. To send plain prompt
text, add `--prompt TEXT` before `--`; choose a configured provider and model in
the child arguments. Prompting can incur provider charges. The example cancels
dialogs and stops after the first run ends; it does not track later automatic
retries or compaction recovery, or wait for slash commands handled without a run.

## Check the example

The example's checks are Rust tests beside the terminal client, and their peers
are native Rust fixtures, so only `rpc.py` itself runs under Python 3:

```sh
just prepare-tests
cargo nextest run --locked -p maestro-sdk-examples -E 'binary(rpc_extension_ui)'
```

This command runs all 17 terminal client example tests, including the SDK checks.
The SDK checks cover LF framing with Unicode separators, startup dialogs, response
correlation, run completion in either order, refusals, end of input and child
exit status. They need neither Maestro nor credentials.

## Run an in-process sample

The thirteen `.rs` samples above use the session SDK in-process, with Maestro's
scripted model and disposable configuration. They make no paid request and
need no credentials. Run one from the repository root:

```sh
cargo run -p maestro-sdk-examples --example minimal
```

The [embedding examples guide](../../docs/application/examples.md) explains
what each sample shows. The `support/` folder holds their shared setup and
`tests/` their larger checks. Each sample's tests run with the others:

```sh
cargo nextest run -p maestro-sdk-examples
```

After `just prepare-tests`, that command also runs the RPC client checks above
and the interactive client demo's checks against a native test peer.

## Drive an interactive RPC session

[`rpc_client_demo`](rpc_client_demo/main.rs) streams a session's output, reads
one prompt per line, and stops on `exit`. An interrupt aborts the running turn,
or quits when none runs. Pass the path of a built Maestro executable:

```sh
cargo run -p maestro-sdk-examples --example rpc_client_demo -- path/to/maestro
```

The demo selects a provider and model, so its prompts need credentials and
can incur charges.

Other SDK examples are listed in the
[examples tracking issue](https://github.com/Orchestration-Maestro/maestro/issues/2074).

## Shipped support modules

These files belong to the [terminal RPC client](../rpc_extension_ui.rs). They
compile through its single registered entry point, not as separate examples.

| File | Purpose |
| --- | --- |
| `rpc_extension_ui/client.rs` | Retained widgets, streamed records, correlated commands and extension dialogs. |
| `rpc_extension_ui/output_log.rs` | Bounded output, literal model text and cell-aware viewport clipping. |

## Running

The client starts `target/debug/maestro` (`maestro.exe` on Windows) and loads
the companion extension `target/wasm32-wasip2/release/examples/rpc_demo.wasm`,
both under the directory that `MAESTRO_PACKAGE_DIR` names. Without that
variable it looks in the Maestro sources Cargo fetched for this workspace,
where neither file is built.

Build both in a Maestro checkout at the
[revision this repository pins](../../docs/examples.md#the-maestro-pin), with
the pinned Rust toolchain and the `wasm32-wasip2` target:

```sh
rustup target add wasm32-wasip2
CARGO_TARGET_DIR=target cargo build --locked -p maestro
CARGO_TARGET_DIR=target cargo build --locked \
  -p maestro-extensions-wasm --example rpc_demo --target wasm32-wasip2 --release
```

Then run the client from this repository's root, pointing it at that checkout:

```sh
MAESTRO_PACKAGE_DIR=path/to/maestro cargo run --locked \
  -p maestro-sdk-examples --example rpc_extension_ui
```

The backend runs without saved session history and loads only the companion
extension.
