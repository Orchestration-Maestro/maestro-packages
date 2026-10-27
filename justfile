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
