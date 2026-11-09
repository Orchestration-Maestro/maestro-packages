# Security test libraries

The security fixtures use `socket2 =0.6.5` as a Unix-only development dependency,
with default features disabled and `all` enabled for its safe socket-mode getter.
The assertions verify blocking accepted streams before reading requests; other
platforms retain the request-capture and response-delivery tests.
The crate is licensed under MIT OR Apache-2.0. No production dependency changes.

The extension tests use `indexmap =2.14.2`, Maestro's exact pin, as a
development dependency only. Their stand-in host names the ordered tool maps
that two host methods return. The crate is licensed under MIT OR Apache-2.0
and was already in the lock; no production dependency changes.
