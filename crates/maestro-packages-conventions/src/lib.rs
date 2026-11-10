//! Repository checks that cargo and Clippy do not make: every workspace member inherits the
//! workspace lint table, and git tracks only Rust sources.

use std::path::Path;

/// Members that must inherit the workspace lint table.
mod lints;
/// Tracked files that are not Rust sources.
mod tracked_sources;
/// The failures the checks report.
mod violation;

pub use violation::Violation;

/// Run every repository check on the workspace at `root` and report all failures together.
///
/// # Errors
/// Returns every [`Violation`] found: a member that does not inherit the workspace lints, a
/// tracked file in a language other than Rust, or a manifest or repository that cannot be read.
pub fn check_workspace(root: &Path) -> Result<(), Vec<Violation>> {
    let mut violations = Vec::new();
    for outcome in [lints::check(root), tracked_sources::check(root)] {
        match outcome {
            Ok(found) => violations.extend(found),
            Err(stopped) => violations.push(stopped),
        }
    }
    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}
