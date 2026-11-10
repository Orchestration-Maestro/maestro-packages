# Dynamic resources

Quick start: build the component, install it beside its three resource files, and load it with `maestro -e <path>`. Maestro then lists the `dynamic-resources` skill, the `/dynamic` prompt template and the `dynamic-resources` theme.

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo build --locked \
  -p maestro-extension-examples --example dynamic_resources --target wasm32-wasip2 --release
```

## How it works

[`dynamic-resources/main.rs`](dynamic-resources/main.rs) answers `resources_discover` with the files that sit next to the installed component:

- [`SKILL.md`](dynamic-resources/SKILL.md), the skill
- [`dynamic.md`](dynamic-resources/dynamic.md), the prompt template
- [`dynamic.json`](dynamic-resources/dynamic.json), the theme

The paths come from the installed component, not from the session's working directory. Startup and reload read the files as they are on disk. Keep the four files together when you move the component.

## Install and load

Build with the pinned toolchain and the `wasm32-wasip2` target, as in the quick start. Copy `dynamic_resources.wasm` into a folder with the three resource files, then pass the component with `maestro -e <path>`. Loading is opt-in and needs Maestro's native component runtime. Maestro sets `MAESTRO_EXTENSION_PATH` for each instance; leave it unset.

Discovery and reload make no model call. Sending the expanded `/dynamic` prompt uses the session's configured provider.

## Controlled run

`dynamic_resources_component_returns_shared_paths` installs the built component at several disposable locations and starts it from unrelated working directories. The locations include one with spaces and a non-ASCII name and, on Unix, one with a backslash in the file name. At startup and after each reload, the component must return the paths beside it. The run needs no live service and no credentials.

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped just prepare-tests
TEST_PREPARE_OUTPUT="$PWD/target/test-setup" CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- \
  cargo nextest run --locked -p maestro-extension-examples -E 'test(dynamic_resources)'
```
