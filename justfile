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
    just _check --workspace

# Shared checks for explicit Cargo package-selection arguments.
[private]
_check *selection:
    cargo test -p maestro-packages-conventions --locked --test integration -- --exact real_workspace::this_workspace_inherits_lints_and_tracks_only_rust
    cargo clippy "$@" --all-targets --locked -- -D warnings
    RUSTDOCFLAGS="-D warnings -D missing_docs" cargo doc "$@" --no-deps --locked

# Install Maestro's development tool at the pinned commit, then format the staged crates, re-stage the captured paths and check them.
pre-commit:
    cargo install --quiet --locked --git https://github.com/Orchestration-Maestro/maestro --rev 5ac4b8926090b5fcf41e33433b8456720bd3569e maestro-tooling --bin development --root target/development
    target/development/bin/development pre-commit

# Run all workspace tests.
test *args:
    cargo nextest run --workspace --locked "$@"
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
