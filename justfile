set positional-arguments

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
    python3 guard-lints.py
    python3 -m unittest discover -s scripts -p 'test_*.py'
    cargo clippy "$@" --all-targets --locked -- -D warnings
    RUSTDOCFLAGS="-D warnings -D missing_docs" cargo doc "$@" --no-deps --locked

# Capture the index before formatting and verify only the captured scope.
pre-commit:
    python3 scripts/pre_commit.py

# Run all workspace tests.
test *args:
    cargo nextest run --workspace --locked "$@"
    cargo test --workspace --locked --doc "$@"

# Build the runnable browser chat example, then serve it on loopback.
browser-chat-example *args:
    cargo build --locked -p maestro-web-example --example chat --target wasm32-unknown-unknown --release
    wasm-bindgen {{quote(justfile_directory() / "target/wasm32-unknown-unknown/release/examples/chat.wasm")}} --target web --out-dir {{quote(justfile_directory() / "target/maestro-browser/chat")}} --out-name chat
    cp {{quote(justfile_directory() / "examples/web/chat/index.html")}} {{quote(justfile_directory() / "target/maestro-browser/chat/index.html")}}
    @miniserve {{quote(justfile_directory() / "target/maestro-browser/chat")}} --interfaces 127.0.0.1 --index index.html {{quote(args)}}

# Run every browser test of the chat example in the supplied Chromium, even after a failure.
browser-test:
    cargo nextest run --locked -p maestro-web-example --run-ignored only --no-fail-fast -E 'test(/^browser::/)'
