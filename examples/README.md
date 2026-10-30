# Non-Rust guest toolchains

Tracking: [#41](https://github.com/Orchestration-Maestro/maestro-packages/issues/41) and
[#56](https://github.com/Orchestration-Maestro/maestro-packages/issues/56).

- [TypeScript](typescript/README.md): jco 1.40.0 builds a component from the unchanged
  extension contract, and Maestro's host installs and runs it. This is a working extension.
- [Python](python/README.md): componentize-py 0.25.1 builds a valid component,
  but its async borrowed callback traps in the matching host.

The Python sources reproduce its failure and are not a working extension. No Rust guest
SDK or altered WIT is shipped.

## World inspected

Repository: <https://github.com/Orchestration-Maestro/maestro>

Revision: `d3f7aa6a9906d23350f4b28a88a41e0fde8195a0`

World file: `crates/maestro-extensions-wasm/wit/world.wit`

World: `maestro:extension/extension@0.1.0`, importing `host` and exporting `guest`.
Use the complete `wit/` directory, including `standard/`, unchanged.

SHA-256 of `world.wit`:

```text
1a9704e411ee0bcc01cf19e9003c3560bb82c50f12927d5d41c8aefd1f6b7893
```

All 12 files in the WIT directory matched between the inspected checkout and
build host. The guest guide at `docs/extensions/guest.md` in that revision
requires callback registration, async event/tool invocation and callback release.
Generating declarations alone does not qualify an extension against this world.

The TypeScript example ran against a host built from a later revision,
`c9db49dc50b938c8804c9af29f21d7780691c49d`, whose WIT directory is byte-identical.

## Matching host

Python's runtime failure was reproduced with Maestro built from the exact
revision above, not an older executable with missing imports. The accepted
remote build receipt records:

```json
{
  "id": "08997e498993-1793194765783818372",
  "digest": "7858313715aac49ecb4a81c2b5c5ae2ad66f196934426fd71c27e9ecdc0574de",
  "argv": ["mise", "exec", "--", "cargo", "build", "--release", "--locked", "-p", "maestro"],
  "exit": 0,
  "started_at": "2026-10-28T13:39:26Z",
  "rustc": "rustc 1.98.0 (88d9e12ae 2026-08-18)",
  "cargo": "cargo 1.98.0 (797e8a9bc 2026-08-05)"
}
```

The request, receipt, local input snapshot and remote snapshot digests matched
before artifact retrieval. The executable reports `0.1.0`; its SHA-256 is:

```text
59fe05ea4fdbefb99c6b8e749286887c23aace22142b9d0ea6941696c0f3e77f
```

An earlier interrupted build had no valid receipt and was not accepted.

## Verification environment

All component compilation for the diagnostics ran on the remote build host through
the approved build wrapper on 2026-10-28. Observed versions: jco 1.39.0, componentize-py
0.25.1 and wasm-tools 1.262.0 (`e22db44da`, 2026-10-13). The provisioned Node.js
version is 24.20.0. The TypeScript example was built and run on 2026-10-29 with jco
1.40.0, which the build host now provides. The wasi-sdk compiler reports `clang version
23.1.0-wasi-sdk`, installed under the wasi-sdk 34.0 distribution; neither
the TypeScript example nor the Python diagnostic invokes it directly.

Build-machine commands in the language reports use the tool CLIs directly.
Workspace verification used the approved remote wrapper and its artifact
output directory. All source WIT files remained read-only.
