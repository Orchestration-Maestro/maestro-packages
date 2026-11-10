//! Repository checks that cargo and Clippy do not make: every workspace member inherits the
//! workspace lint table, and git tracks only Rust sources.

use std::path::Path;

/// Members that must inherit the workspace lint table.
mod lints;
/// Tracked files that are not Rust sources.
mod tracked_sources;

/// Run every repository check on the workspace at `root` and report all failures together.
///
/// # Errors
/// Returns one message per failed check: a member that does not inherit the workspace lints,
/// a tracked `.py`, `.js`, `.ts` or `.sh` file, or a manifest or repository that cannot be read.
pub fn check_workspace(root: &Path) -> Result<(), String> {
    let failures: Vec<String> = [lints::check(root), tracked_sources::check(root)]
        .into_iter()
        .filter_map(Result::err)
        .collect();
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}
