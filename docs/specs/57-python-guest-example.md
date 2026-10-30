# Python guest example that runs in the host

Issue: [#57](https://github.com/Orchestration-Maestro/maestro-packages/issues/57).
Follows [#41](https://github.com/Orchestration-Maestro/maestro-packages/issues/41),
which closed with a blocker report.

## Contract

`examples/python` holds one Python component for `maestro:extension/extension@0.1.0`.
It registers an `input` hook and a `hello` tool. The world, its dependencies and the
host stay unchanged.

A Maestro host built from source loads the component and a Rust test shows both
registrations working: the model request carries the hook's output, and the model
receives the tool's result. The test is the `maestro-python-example` crate, which
runs the host steps from the shared `tests/host` module.

## Toolchain

componentize-py 0.25.1 builds the component but cannot run it: its runtime calls
`task.return` before it releases the borrowed handles lent to the task. The
component model requires borrows to be released first, and the host enforces that.
The fix belongs to the toolchain, so this work ships a one-hunk patch to the
toolchain's runtime and the exact recipe that builds a patched compiler.

Do not change the world or the host to avoid the toolchain defect.

## Verification

Red first: build the example with the unpatched compiler and run the test. It
fails with `borrow handles still remain at the end of the call`. Build it with the
patched compiler and the same test passes. The README records both runs. Check
all public text with the repository's public-text guard.

## Scope

Examples, their documentation and the toolchain patch change. The Rust crates, the
core WIT and the host stay untouched. The diagnostic that only reproduced the
failure is removed because the example reproduces it.
