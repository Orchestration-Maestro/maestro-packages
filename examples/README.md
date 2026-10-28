# Non-Rust guest examples: verification blocked

Tracking: [#41](https://github.com/Orchestration-Maestro/maestro-packages/issues/41).

Neither language has a verified example yet. The approved remote build route
rejects both toolchain probes before starting them. This is an infrastructure
blocker, **not** evidence that either toolchain rejects the extension world.
No component, guessed bindings or copied Rust guest SDK is shipped here.

- [TypeScript probe](typescript/README.md): jco 1.39.0, experimental guest types.
- [Python probe](python/README.md): componentize-py 0.25.1.

These versions come from the npm and PyPI registry metadata, respectively.
They are pinned probe targets, not versions observed running on the build host.
Runtime and package-manager versions remain unmeasured there.

## World inspected

Repository: <https://github.com/Orchestration-Maestro/maestro>

Revision: `d3f7aa6a9906d23350f4b28a88a41e0fde8195a0`

World file: `crates/maestro-extensions-wasm/wit/world.wit`

World: `maestro:extension/extension@0.1.0`, importing `host` and exporting `guest`.
Use the complete `wit/` directory, including `standard/`, unchanged; the world
file alone does not define its interfaces.

SHA-256 of `world.wit`:

```text
1a9704e411ee0bcc01cf19e9003c3560bb82c50f12927d5d41c8aefd1f6b7893
```

The inspected `guest.wit` exports async `start`, `invoke-event` and
`invoke-tool`. `host.wit` imports synchronous `on` and async `register-tool`.
A synchronous replacement world would not satisfy this contract. TypeScript
Promises or Python coroutines alone do not prove component ABI compatibility.

The guest guide at `docs/extensions/guest.md` in that revision describes these
same registration, event and tool surfaces. A working example must register an
event callback and tool from `start`, keep their callback resources alive, serve
`invoke-event` and `invoke-tool`, and honor `release`. Binding generation alone
will not establish that the component works in the host.

## Observed result

Both pinned probes were refused on 2026-10-28 at 12:28 UTC with exit code 1:

```text
maestro-remote-build: not remote eligible
```

The wrapper currently admits Cargo checks and selected recipes, not npm or
Python toolchain commands. No language parser or compiler ran. No local build
or direct remote-shell workaround was attempted.

Build command and output: unavailable until the probes run and bindings can be
implemented and tested. Install command: unavailable because no component was
built. Run output: unavailable because nothing was installed. The linked
language reports give the exact first commands to retry through the approved
remote route; they are probes, not successful build recipes.

## Remaining acceptance work

1. Enable the approved remote route for both pinned toolchains and record their
   actual runtime versions and probe output.
2. If a probe fails on the WIT, record the exact toolchain error and version.
   Otherwise implement the generated world as-is and test event and tool calls.
3. Publish only verified components with exact build, install and controlled-run
   commands and observed output. Keep experimental labels where required.
