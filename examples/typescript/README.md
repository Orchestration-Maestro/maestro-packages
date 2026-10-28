# TypeScript: component compiler blocked

**Diagnostic only, not a working extension.** Experimental toolchain:
`@bytecodealliance/jco` **1.39.0**, with provisioned Node.js **24.20.0**.
`jco guest-types --help` labels guest types experimental. Install the pinned
tool on your build machine with `npm install -g @bytecodealliance/jco@1.39.0`.

`compiler_diagnostic.ts` supplies an empty guest object to isolate WIT code
generation. It is deliberately not an extension implementation. Both compiler
backends fail on the unchanged world before it can become a working example.

## Reproduce

Use the complete [extension world](../README.md#world-inspected) at Maestro
revision `d3f7aa6a9906d23350f4b28a88a41e0fde8195a0`.
The [matching-host receipt](../README.md#matching-host) pins the executable used
to qualify that revision's world for the Python runtime check. TypeScript's
compiler fails before any host invocation.

From this repository with that sibling Maestro checkout:

```sh
WIT="$PWD/../maestro/crates/maestro-extensions-wasm/wit"
jco --version
jco guest-types "$WIT" --world-name extension --out-dir generated
cd examples/typescript
jco componentize compiler_diagnostic.ts \
  --wit "$WIT" --world-name extension --out typescript.wasm
```

Workspace verification ran through the approved remote build wrapper on
2026-10-28. Version output is `1.39.0`. Declaration generation exits 0 and
writes `extension.d.ts` and the guest, host and session interface declarations.
This does not establish component compiler support.

The default StarlingMonkey backend exits 1 during binding generation:

```text
thread '<unnamed>' (1) panicked at crates/spidermonkey-embedding-splicer/src/bindgen.rs:721:48:
not yet implemented
RuntimeError: unreachable
```

Still in the TypeScript directory, the QuickJS backend also exits 1:

```sh
jco componentize compiler_diagnostic.ts --backend quickjs \
  --wit "$WIT" --world-name extension --out typescript-qjs.wasm
```

```text
(jco componentize) [Error: component imports instance `maestro:extension/host@0.1.0`, but a matching implementation was not found in the linker: instance export `[method]file-mutation.wait` has the wrong type: type mismatch with async] {
  code: 'GenericFailure'
}
```

Both commands use the full world. Neither changes async functions to
synchronous functions. The QuickJS error names the async resource method it
cannot link. The default backend's panic alone does not identify which WIT
declaration caused it.

## Build, install and run

Build: blocked by the component compiler errors above, despite successful
declaration generation. Install command: none, because neither backend produced
a component. Run output: none, because there is no component to install.

Retry both backends after a toolchain update. A working example must register
an event handler and a tool, retain their callbacks until release, and pass
host invocations against the unchanged world before it gains an install recipe.
