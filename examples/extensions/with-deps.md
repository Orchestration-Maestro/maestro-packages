# With deps

The [`with-deps/main.rs`](with-deps/main.rs) example parses a duration string
with a regular expression. The dependency is the workspace pin, declared once as
a development dependency of this crate, so it reaches the crate's examples and
tests without entering the guest library; only the pinned toolchain and the
`wasm32-wasip2` target are needed:

```sh
cargo build --locked \
  -p maestro-extension-examples --example with_deps --target wasm32-wasip2 --release
```

Its controlled run, listed below, calls the registered tool with
every recorded duration text.

## Controlled runs

```sh
just prepare-tests
TEST_PREPARE_OUTPUT="$PWD/target/test-setup" cargo nextest run --locked \
  -p maestro-extension-examples -E 'test(with_deps)'
```
