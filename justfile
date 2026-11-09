set positional-arguments

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
    python3 guard-lints.py
    python3 -m unittest discover -s scripts -p 'test_*.py'
    cargo clippy "$@" --all-targets --locked -- -D warnings
    RUSTDOCFLAGS="-D warnings -D missing_docs" cargo doc "$@" --no-deps --locked

# Capture the index before formatting and verify only the captured scope.
pre-commit:
    python3 scripts/pre_commit.py

# Install Maestro's development tool from the pinned commit into `target/development`.
[private]
development:
    cargo install --quiet --locked --git https://github.com/Orchestration-Maestro/maestro --rev 1a57f478148aa8b88a52a265b6f02aa467bc8d0f maestro-tooling --bin development --root target/development

# Build the example components and install the programs the tests start into `target/test-setup`.
prepare-tests: development
    target/development/bin/development prepare-test-setup prepare target/test-setup "${CARGO_TARGET_DIR:-target/test-prepare}"

# Render the extension index, the example targets and the guide lock.
example-index: development
    target/development/bin/development example-index

# Check the rendered extension files without rewriting them.
example-index-check: development
    target/development/bin/development example-index --check

# Run all workspace tests.
test *args: prepare-tests
    cargo nextest run --workspace --locked "$@"
    cargo test --workspace --locked --doc "$@"
