//! Baseline public-surface tests for security assessment capabilities.

#[path = "capabilities/audit.rs"]
mod audit;
#[path = "capabilities/bench.rs"]
mod bench;
#[path = "capabilities/coverage.rs"]
mod coverage;
#[path = "capabilities/doctor.rs"]
mod doctor;
#[path = "capabilities/report.rs"]
mod report;
mod support;
