# TypeScript guest: remote probe blocked

**Experimental, not a working example.** Target toolchain:
`@bytecodealliance/jco` **1.39.0**. Its
[pinned CLI source](https://github.com/bytecodealliance/jco/tree/c63b86fc64448a0bc5a43038ebabf4488c7e3a57)
labels `guest-types` experimental. No Node.js, npm or jco version was measured
on the build host. Do not interpret this report as a language incompatibility.

## Reproduce the blocked probe

On the build client, set `WIT` to the complete, unchanged directory documented
in the [world report](../README.md#world-inspected). Run from this repository with a sibling Maestro checkout at that revision:

```sh
WIT="$PWD/../maestro/crates/maestro-extensions-wasm/wit"
maestro-remote-build run npm exec --yes \
  --package=@bytecodealliance/jco@1.39.0 -- \
  jco guest-types "$WIT" --world-name extension \
  --out-dir generated/typescript
```

Observed output, before npm or jco started; exit code 1:

```text
maestro-remote-build: not remote eligible
```

The remote route must admit this command and make the pinned WIT directory
available before it can establish toolchain support. No local fallback is
allowed. Generated guest types would be only the first check: the actual
component must implement the unchanged async world and pass host event/tool
invocations, not just TypeScript type checking.

## Build, install and run

Build command: not yet verified. Install command: none; no component exists.
Run output: none; no component was installed. Shipping source or an install
recipe now would suggest compatibility that has not been tested.

The [shared report](../README.md) records the WIT revision, required surfaces
and remaining verification steps.
