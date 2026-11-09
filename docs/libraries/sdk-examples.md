# SDK example libraries

The SDK samples' image test decodes a one-pixel PNG with `base64 =0.23.1`,
Maestro's exact pin, as a development dependency with default features
disabled and `std` enabled. The crate is licensed under MIT OR Apache-2.0 and
was already in Maestro's lock through its application crate. No production
dependency changes.

The examples build on Maestro's own crates at the pinned commit, through the
root workspace entries, and on `serde_json` and `tokio` as the workspace
declares them. `tokio` adds `sync` and `process` for the RPC clients.
