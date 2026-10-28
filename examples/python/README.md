# Python guest: remote probe blocked

**Not a working example.** Target toolchain: `componentize-py` **0.25.1**.
Its [pinned source](https://github.com/bytecodealliance/componentize-py/tree/v0.25.1)
includes WASI P3 examples, so async declarations alone do not establish an
unsupported-world blocker. No Python, uv or componentize-py version was
measured on the build host.

## Reproduce the blocked probe

On the build client, set `WIT` to the complete, unchanged directory documented
in the [world report](../README.md#world-inspected). Run from this repository with a sibling Maestro checkout at that revision:

```sh
WIT="$PWD/../maestro/crates/maestro-extensions-wasm/wit"
maestro-remote-build run uvx --from componentize-py==0.25.1 \
  componentize-py -d "$WIT" -w extension \
  bindings generated/python
```

Observed output, before uv or componentize-py started; exit code 1:

```text
maestro-remote-build: not remote eligible
```

The remote route must admit this command and make the pinned WIT directory
available before it can establish toolchain support. No local fallback is
allowed. Generated bindings would be only the first check: the actual component
must implement the complete async guest interface and pass host event/tool
invocations. Python-only tests would not establish ABI compatibility.

## Build, install and run

Build command: not yet verified. Install command: none; no component exists.
Run output: none; no component was installed. No language parser rejection is
claimed, and no unverified Python extension is shipped.

The [shared report](../README.md) records the WIT revision, required surfaces
and remaining verification steps.
