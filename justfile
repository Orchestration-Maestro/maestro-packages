set positional-arguments

# Cargo's artifact directory: the build, the binding and the serving step all read it here.
target := env("CARGO_TARGET_DIR", justfile_directory() / "target")
# The command that serves the bound example; a test replaces it to see the arguments it gets.
serve := "miniserve"

# Install the pinned tools and all commit hook stages.
setup:
    mise install
    mise exec -- prek install

# Formatting, lint, public documentation and the workspace conventions.
check:
    cargo fmt --all
    just example-index-check
    just _check --workspace

# Shared checks for explicit Cargo package-selection arguments.
[private]
_check *selection:
    cargo test -p maestro-packages-conventions --locked --test integration -- --exact real_workspace::this_workspace_inherits_lints_and_tracks_only_rust
    cargo clippy "$@" --all-targets --locked -- -D warnings
    RUSTDOCFLAGS="-D warnings -D missing_docs" cargo doc "$@" --no-deps --locked

# Format the staged crates, re-stage the captured paths and check them with the pinned development tool.
pre-commit: development
    target/development/bin/development pre-commit

# Install Maestro's development tool from the pinned commit into `target/development`.
[private]
development:
    cargo install --quiet --locked --git https://github.com/Orchestration-Maestro/maestro --rev b5b1f350af56b76c97290dd0a40959e33f6174c5 maestro-tooling --bin development --root target/development

# Build the example components and install the programs the tests start into `target/test-setup`.
prepare-tests: development
    target/development/bin/development prepare-test-setup prepare target/test-setup "${CARGO_TARGET_DIR:-target/test-prepare}"

# Render the extension index, the example targets and the guide lock.
example-index: development
    target/development/bin/development example-index

# Check the rendered extension files without rewriting them.
example-index-check: development
    target/development/bin/development example-index --check

# Run all workspace tests against the bundle that `prepare-tests` staged.
test *args: prepare-tests
    TEST_PREPARE_OUTPUT={{quote(justfile_directory() / "target/test-setup")}} cargo nextest run --workspace --locked "$@"
    cargo test --workspace --locked --doc "$@"

# Build the runnable browser chat example for the browser and bind it; the shared browser build runs this recipe.
browser-build:
    cargo build --locked --target-dir {{quote(target)}} -p maestro-web-example --example chat --target wasm32-unknown-unknown --release
    wasm-bindgen {{quote(target / "wasm32-unknown-unknown/release/examples/chat.wasm")}} --target web --out-dir {{quote(target / "maestro-browser/chat")}} --out-name chat
    cp {{quote(justfile_directory() / "examples/web/chat/index.html")}} {{quote(target / "maestro-browser/chat/index.html")}}

# Build the browser chat example, then serve it on loopback; extra arguments reach the server.
browser-chat-example *args: browser-build
    @just browser-chat-serve "$@"

# Serve the bound browser chat example on loopback; extra arguments reach the server.
browser-chat-serve *args:
    @{{quote(serve)}} {{quote(target / "maestro-browser/chat")}} --interfaces 127.0.0.1 --index index.html "$@"

# Run every browser test of the chat example in the supplied Chromium, even after a failure.
browser-test:
    cargo nextest run --locked -p maestro-web-example --run-ignored only --no-fail-fast -E 'test(/^browser::/)'
