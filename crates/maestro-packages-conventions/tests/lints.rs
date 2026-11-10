//! Lint inheritance of workspace members.

use maestro_packages_conventions::check_workspace;

use crate::support::Workspace;

/// A member manifest that inherits the workspace lints.
const INHERITING: &str = "[package]\nname = \"inheriting\"\n\n[lints]\nworkspace = true\n";

#[test]
fn member_inheriting_workspace_lints_passes() {
    let workspace = Workspace::new();
    workspace.member("inheriting", INHERITING);
    workspace.member(
        "dotted",
        "lints.workspace = true\n\n[package]\nname = \"dotted\"\n",
    );

    let result = check_workspace(workspace.path());

    assert_eq!(result, Ok(()));
}

#[test]
fn member_without_a_lints_table_is_reported() {
    let workspace = Workspace::new();
    workspace.member("bare", "[package]\nname = \"bare\"\n");

    let error = check_workspace(workspace.path()).unwrap_err();

    assert!(error.contains("crates/bare/Cargo.toml"), "{error}");
}

#[test]
fn member_with_only_its_own_lint_levels_is_reported() {
    let workspace = Workspace::new();
    workspace.member(
        "local",
        "[package]\nname = \"local\"\n\n[lints.rust]\nunsafe_code = \"forbid\"\n",
    );

    let error = check_workspace(workspace.path()).unwrap_err();

    assert!(error.contains("crates/local/Cargo.toml"), "{error}");
}

#[test]
fn every_failing_member_is_named_and_inheriting_members_are_not() {
    let workspace = Workspace::new();
    workspace.member("inheriting", INHERITING);
    workspace.member("first", "[package]\nname = \"first\"\n");
    workspace.member("second", "[package]\nname = \"second\"\n");

    let error = check_workspace(workspace.path()).unwrap_err();

    assert!(error.contains("crates/first/Cargo.toml"), "{error}");
    assert!(error.contains("crates/second/Cargo.toml"), "{error}");
    assert!(!error.contains("inheriting"), "{error}");
}

#[test]
fn literal_member_path_is_checked() {
    let workspace = Workspace::new();
    workspace.write("Cargo.toml", "[workspace]\nmembers = [\"tools/one\"]\n");
    workspace.write("tools/one/Cargo.toml", "[package]\nname = \"one\"\n");

    let error = check_workspace(workspace.path()).unwrap_err();

    assert!(error.contains("tools/one/Cargo.toml"), "{error}");
}

#[test]
fn directory_without_a_manifest_is_not_a_member() {
    let workspace = Workspace::new();
    workspace.write("crates/notes/README.md", "no manifest\n");

    assert_eq!(check_workspace(workspace.path()), Ok(()));
}

#[test]
fn member_pattern_other_than_a_directory_glob_is_refused() {
    let workspace = Workspace::new();
    workspace.write(
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/*/inner\"]\n",
    );

    let error = check_workspace(workspace.path()).unwrap_err();

    assert!(error.contains("crates/*/inner"), "{error}");
}

#[test]
fn malformed_member_manifest_is_reported_with_its_path() {
    let workspace = Workspace::new();
    workspace.member("broken", "[package\nname = \"broken\"\n");

    let error = check_workspace(workspace.path()).unwrap_err();

    assert!(error.contains("crates/broken/Cargo.toml"), "{error}");
}

#[test]
fn glob_over_a_missing_directory_has_no_members() {
    let workspace = Workspace::new();

    assert_eq!(check_workspace(workspace.path()), Ok(()));
}
