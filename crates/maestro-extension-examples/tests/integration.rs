//! The extension examples, loaded in Maestro's native host and executable.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

mod executable;
mod host;
mod shipped_examples;
mod source;
