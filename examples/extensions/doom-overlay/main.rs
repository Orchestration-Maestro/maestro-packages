//! Play DOOM in a retained terminal overlay.
// maestro-component: MAESTRO_DOOM_OVERLAY_COMPONENT
#![cfg(target_arch = "wasm32")]
mod doom_automap;
mod doom_cheats;
pub mod doom_component;
pub mod doom_engine;
pub mod doom_keys;
mod doom_screen;
mod factory;
pub mod wad_check;
pub mod wad_finder;
pub use factory::factory;

maestro_extensions_wasm::export_extension!(factory);
