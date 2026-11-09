# Browser example libraries

The browser chat example and its tests use the exact pins Maestro declares at the
pinned revision: `wasm-bindgen =0.2.129`, `wasm-bindgen-futures =0.4.79`,
`js-sys =0.3.106` and `web-sys =0.3.106`, all MIT OR Apache-2.0. They are
development dependencies of `maestro-web-example`, enabled only for
`wasm32-unknown-unknown`, and they were already in the lock through
`maestro-web`.

The native test driver comes from Maestro's `maestro-test-browser` and
`maestro-tooling` crates at the pinned revision. They bring the browser driver
libraries Maestro already approves for that job: `chromiumoxide 0.9.1`, `hyper`,
`hyper-util`, `reqwest` and `sha2`, among others. The crate adds none of its own.

Mise pins two command-line tools: `wasm-bindgen` 0.2.129 (MIT OR Apache-2.0), which
must match the locked library, and `miniserve` 0.35.0 (MIT). Their release digests are
in `mise.lock`. Chromium is supplied by the shared browser job as revision 1248.
