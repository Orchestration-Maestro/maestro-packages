//! Maestro's unified security framework: understand program and network
//! behaviour, find security issues with root-cause analysis, and write better
//! code.
//!
//! One pipeline (recon, target model, hypothesis sweep, verify, deepen,
//! threat, report) with five agents, driven by a model resolved at run time
//! from the bridge and executed through the analysis sandbox runner. Every
//! step is ledgered under a sequential evidence id and the full, untruncated
//! outputs are retained in one ledger file per run.
//!
//! The pipeline is extended by a state backbone (run index, workspaces,
//! semantic memory), orchestration (campaigns, named workspaces), capabilities
//! (attack surface, source hunt, reverse engineering, audit, browser, exploit),
//! and operations (remediation, disclosure, benchmarking, call-graph
//! visualization, web report, doctor, model map, prestage).
//!
//! Two surfaces expose the same tools:
//! - the wasm extension, registered with Maestro
//! - the MCP stdio server (`maestro-security-mcp`)

mod agents;
pub mod audit;
pub mod autonomy;
pub mod bench;
pub mod browser;
pub mod campaign;
mod ctx;
mod coverage;
pub mod disclose;
pub mod doctor;
pub mod exploit;
mod evidence;
mod llm;
mod memory;
mod methodology;
pub mod models;
mod phases;
mod pipeline;
pub mod prestage;
pub mod remediate;
mod report;
pub mod reverse;
mod router;
mod runner;
pub mod sourcehunt;
mod store;
pub mod surface;
mod tools;
mod validate;
pub mod visualize;
mod webui;
pub mod workspace;

#[cfg(any(target_arch = "wasm32", test))]
mod extension;
// The MCP stdio surface is host-side; the WASM surface is the extension.
#[cfg(not(target_family = "wasm"))]
pub mod mcp;

pub use agents::{resolve_agent, Agent, PANEL, Pattern, ROSTER};
pub use audit::{Check, Scope, Report};
pub use bench::{Suite, Scorecard};
pub use browser::{Visit, Capture, PageInfo};
pub use campaign::{Targets, Tally};
pub use disclose::{Brief, Disclosure, sarif};
pub use doctor::{Vitals, HealthCheck};
pub use coverage::{Attempt, CoverageDoc, CoverageStatus, CoverageUnit};
pub use evidence::{base_dir, load_report, save_report, Ledger, LedgerDoc};
pub use exploit::{ChainRun, Playbook, Outcome};
pub use llm::Client;
pub use memory::Memory;
pub use models::{ModelMap, RoleModel};
pub use pipeline::{assess, evidence_for, AssessParams, Config, Engine, RunOptions};
pub use prestage::{ImageStatus, Manifest};
pub use remediate::{Ticket, Plan, Remediation};
pub use report::{Assessment, Finding, RunSpend, SCHEMA, Verdict};
pub use reverse::{Artifact, Analysis};
pub use router::{Domain, DomainsDoc, Router};
pub use sourcehunt::{Hit, Query, Triage};
pub use store::{now_secs, RunRecord, Status, Store, Workspace};
pub use surface::{Inventory, Probe, Reading};
pub use tools::{run_assess, run_doctor, run_evidence, run_evidence_page, run_report_page, run_route, run_validate, run_workspace};
pub use validate::{validate_assessment, validate_coverage, ValidationReport, Violation};
pub use visualize::{CallGraph, Trace};
pub use webui::{render, render_evidence, WebReport, WebuiParams};
pub use workspace::{LogLine, Order, Board};

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(extension::factory);
