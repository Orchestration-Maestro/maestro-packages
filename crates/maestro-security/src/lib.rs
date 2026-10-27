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
pub mod bench;
pub mod browser;
pub mod campaign;
mod coverage;
mod ctx;
pub mod disclose;
pub mod doctor;
mod evidence;
pub mod exploit;
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

#[cfg(target_arch = "wasm32")]
mod extension;
// The MCP stdio surface is host-side; the WASM surface is the extension.
#[cfg(not(target_family = "wasm"))]
pub mod mcp;

pub use agents::{Agent, PANEL, Pattern, ROSTER, resolve_agent};
pub use audit::{Check, Report, Scope};
pub use bench::{Scorecard, Suite};
pub use browser::{Capture, PageInfo, Visit};
pub use campaign::{Tally, Targets};
pub use coverage::{Attempt, CoverageDoc, CoverageStatus, CoverageUnit};
pub use disclose::{Brief, Disclosure, sarif};
pub use doctor::{HealthCheck, Vitals};
pub use evidence::{Ledger, LedgerDoc, base_dir, load_report, save_report};
pub use exploit::{ChainRun, Outcome, Playbook};
pub use llm::Client;
pub use memory::Memory;
pub use models::{ModelMap, RoleModel};
pub use pipeline::{AssessParams, Config, Engine, RunOptions, assess, evidence_for};
pub use prestage::{ImageStatus, Manifest};
pub use remediate::{Plan, Remediation, Ticket};
pub use report::{Assessment, Finding, RunSpend, SCHEMA, Verdict};
pub use reverse::{Analysis, Artifact};
pub use router::{Domain, DomainsDoc, Router};
pub use sourcehunt::{Hit, Query, Triage};
pub use store::{RunRecord, Status, Store, Workspace, now_secs};
pub use surface::{Inventory, Probe, Reading};
pub use tools::{
    run_assess, run_doctor, run_evidence, run_evidence_page, run_report_page, run_route,
    run_validate, run_workspace,
};
pub use validate::{ValidationReport, Violation, validate_assessment, validate_coverage};
pub use visualize::{CallGraph, Trace};
pub use webui::{WebReport, WebuiParams, render, render_evidence};
pub use workspace::{Board, LogLine, Order};

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(extension::factory);
