# Extension example libraries

The extension examples compile in `maestro-extension-examples` with the
libraries their sources import. Each is Maestro's exact pin, so both locks
resolve one version:

| Crate | Pin | Used by | Licence |
| --- | --- | --- | --- |
| `chrono` | `=0.4.45`, default features off, `std` | `tool_override.rs` timestamps | MIT OR Apache-2.0 |
| `httpdate` | `=1.0.3` | `custom_message_provider` retry dates | MIT OR Apache-2.0 |
| `indexmap` | `=2.14.2` | `tools.rs`, `minimal_mode.rs`, `preset/config.rs` | MIT OR Apache-2.0 |
| `num-traits` | `=0.2.19` | `message_renderer.rs` number conversion | MIT OR Apache-2.0 |
| `regex` | `=1.13.1` | `built_in_tool_renderer.rs`, `tool_override.rs`, `rpc_demo.rs`, `with-deps/main.rs` | MIT OR Apache-2.0 |
| `ryu-js` | `=1.0.3` | `custom_footer.rs` number display | Apache-2.0 OR BSL-1.0 |
| `wit-bindgen` | `=0.62.0`, default features off, `async-spawn` | background tasks in the components (WebAssembly targets only) | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |

All are development dependencies of the examples crate; the crate itself has
no production code. `wasmtime` and `wasmtime-wasi` reach the lock through
Maestro's host crates at the git commit Maestro pins, which is why
`.github/ci.toml` allows the `bytecodealliance/wasmtime` source.
