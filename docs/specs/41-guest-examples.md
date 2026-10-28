# Guest examples for the extension contract

Issue: [#41](https://github.com/Orchestration-Maestro/maestro-packages/issues/41).

## Contract

Provide one small TypeScript component and one Python component targeting
`maestro:extension/extension@0.1.0`. Read the world and its dependencies from
`crates/maestro-extensions-wasm/wit/` in the Maestro repository. Do not change
that world, remove async declarations, or copy the Rust guest SDK.

Each working example registers an event handler and a tool using the surfaces
in the guest guide. Its README pins the toolchain and gives exact build,
install and run commands with observed output. Mark experimental tooling as
experimental.

If a toolchain cannot consume the unchanged world, replace that example with
an exact blocker report naming the version and failed command. A build-route
failure is not evidence that the language toolchain rejects the world. Do not
publish an unverified component as a working example.

## Verification

Run toolchain probes and builds only on the remote build host through the
approved build wrapper. Retry refused probes at ten-minute intervals while
waiting for the build route. Record the world revision and file checksums.
Test authored behavior before implementation where the toolchain can run.
Review documentation for concrete commands, unsupported claims and unnecessary
prose. Check all public text with the repository's public-text guard.

## Scope

Only examples, their documentation and necessary behavior tests change. Core
WIT, the host, build infrastructure and Rust guest code remain untouched.
