set positional-arguments

# Install the pinned tools and the commit hook stages.
setup:
    mise install
    mise exec -- prek install

# Formatting, lint, public documentation and tests.
check:
    cargo fmt --all
    python3 guard-lints.py
    cargo clippy --workspace --all-targets --locked -- -D warnings
    RUSTDOCFLAGS="-D warnings -D missing_docs" cargo doc --workspace --no-deps --locked
check:
    cargo fmt --all
    cargo clippy --workspace --all-targets --locked -- -D warnings
    RUSTDOCFLAGS="-D warnings -D missing_docs" cargo doc --workspace --no-deps --locked

# Run all workspace tests.
test *args:
    cargo nextest run --workspace --locked "$@"
    cargo test --workspace --locked --doc "$@"
