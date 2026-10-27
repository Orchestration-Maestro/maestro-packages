//! The component example target: one `export_extension!` for the host.

#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(crate::factory);
