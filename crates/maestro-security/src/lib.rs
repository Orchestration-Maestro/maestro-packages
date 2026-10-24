//! Maestro.s unified security framework: understand program and network behaviour, find security issues with root-cause analysis, and write better code.
//!
//! One pipeline (recon, target model, hypothesis sweep, verify, deepen,
//! threat, report) with five agents, driven by a model resolved at run
//! time from the bridge and executed through the analysis sandbox runner. Every
//! step is ledgered under a sequential evidence id and the full,
//! untruncated outputs are retained in one ledger file per run.
//!
//! Two surfaces expose the same two tools:
//! - the wasm extension, registered with Maestro
//! - the MCP stdio server (`maestro-security-mcp`)

mod agents;
mod evidence;
mod llm;
mod methodology;
mod net;
mod phases;
mod pipeline;
mod report;
mod runner;
mod tools;

#[cfg(target_arch = "wasm32")]
mod extension;
pub mod mcp;

pub use agents::{resolve_agent, Agent, ROSTER};
pub use evidence::{Ledger, LedgerDoc};
pub use pipeline::{assess, evidence_for, AssessParams, Config, Pipeline};
pub use report::{Report, REPORT_SCHEMA};

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(extension::factory);
