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
| `sha2` | `=0.11.0`, default features off | `doom-overlay/wad_finder/checksum.rs` checks the pinned shareware digest before a cache file is created; `prepare_freedoom` checks the pinned Freedoom archive | MIT OR Apache-2.0 |
| `zip` | `=9.0.5`, default features off, `deflate-flate2` | `prepare_freedoom` reads `freedoom1.wad` from the checked archive (native dependency) | MIT |
| `wit-bindgen` | `=0.62.0`, default features off, `async-spawn` | background tasks in the components (WebAssembly targets only) | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |

`sha2` and `zip` are normal dependencies, because the DOOM overlay and the
`prepare_freedoom` binary use them. The rest are development dependencies.
`wasmtime` and `wasmtime-wasi` reach the lock through Maestro's host crates at
the git commit Maestro pins, which is why `.github/ci.toml` allows the
`bytecodealliance/wasmtime` source.

The crate also lists Maestro's executable package, `maestro`, as a git development
dependency at the same pin. The package has no library target, so Cargo ignores the
dependency when it builds, and it still locks the package at the commit that
`just prepare-tests` installs the executable from. Cargo prints one warning for it on
each invocation.

## DOOM overlay engine

The DOOM overlay compiles room4doom into its component. The workspace pins each
room4doom crate the overlay uses to one revision of the fork that carries the
change that lets the engine build for WebAssembly, and the crates are
development dependencies of the components for WebAssembly targets only:

| Crate | Used by | Licence |
| --- | --- | --- |
| `doom-ui` (feature `software25d`), `game-config`, `gameplay` (feature `software25d`), `gamestate`, `gamestate-traits`, `input`, `math`, `pic-data`, `render-common`, `software25d`, `ui-draw`, `wad` | `doom-overlay/doom_engine.rs`, `doom_screen.rs`, `doom_keys.rs` and `doom_component.rs` | MIT |

Their own dependencies reach the lock through them: `glam` (MIT OR Apache-2.0),
`nanoserde` and `nanoserde-derive` (a git source pinned by the lock, MIT OR
Apache-2.0) and `dirs` (MIT OR Apache-2.0). `.github/ci.toml` allows the two git
sources. `pic-data` builds without its PK3 reader, so the component's graph holds
no `zip`. The pin moves to the upstream revision once the change lands there.
