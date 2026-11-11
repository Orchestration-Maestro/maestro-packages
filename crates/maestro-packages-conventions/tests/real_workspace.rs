//! This repository obeys its own checks.

use std::path::Path;

use maestro_packages_conventions::check_workspace;

#[test]
fn this_workspace_inherits_lints_and_tracks_only_rust() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    check_workspace(&root).unwrap();
}
