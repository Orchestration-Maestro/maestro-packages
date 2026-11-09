//! The RPC client demo's checks against Maestro's native test peers.
#![cfg(test)]

#[cfg(unix)]
#[path = "support/child.rs"]
mod child;
#[cfg(unix)]
#[path = "../../../examples/sdk/rpc_client_demo/interactive.rs"]
mod rpc_client_demo;
#[cfg(unix)]
#[path = "support/rpc_peer.rs"]
mod rpc_peer;
#[cfg(unix)]
mod support;
