//! Behavioural coverage through the public capability APIs and HTTP boundaries.

#[path = "capabilities/disclose.rs"]
mod disclose;
#[path = "capabilities/exploit.rs"]
mod exploit;
#[path = "capabilities/remediate.rs"]
mod remediate;
#[path = "capabilities/reverse.rs"]
mod reverse;
#[path = "capabilities/sourcehunt.rs"]
mod sourcehunt;
#[path = "capabilities/support.rs"]
mod support;

#[path = "capabilities/audit.rs"]
mod audit;
#[path = "capabilities/bench.rs"]
mod bench;
#[path = "capabilities/coverage.rs"]
mod coverage;
#[path = "capabilities/doctor.rs"]
mod doctor;
#[path = "support/handler.rs"]
mod handler_support;
#[path = "capabilities/report.rs"]
mod report;
