//! The chat example's browser exports, compiled as one WebAssembly program that Chromium runs.
#![cfg(all(test, target_arch = "wasm32"))]

#[cfg(test)]
mod brand;
#[cfg(test)]
mod browser_fixture_support;
#[cfg(test)]
mod chat_example_browser;
#[cfg(test)]
mod contributions_support;
#[cfg(test)]
mod dialog_support;
#[cfg(test)]
mod support;
