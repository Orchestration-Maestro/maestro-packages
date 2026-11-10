//! Lint inheritance of workspace members.

use std::path::{Path, PathBuf};

use maestro_packages_conventions::{Violation, check_workspace};

use crate::support::Workspace;

/// A member manifest that inherits the workspace lints.
const INHERITING: &str = "[package]\nname = \"inheriting\"\n\n[lints]\nworkspace = true\n";

/// The manifests of the members reported for not inheriting the workspace lints, in report order.
fn failing_members(violations: &[Violation]) -> Vec<&Path> {
    violations
        .iter()
        .filter_map(|violation| match violation {
            Violation::MemberWithoutWorkspaceLints(manifest) => Some(manifest.as_path()),
            _ => None,
        })
        .collect()
}

#[test]
fn member_inheriting_workspace_lints_passes() {
    let workspace = Workspace::new();
    workspace.member("inheriting", INHERITING);
    workspace.member(
        "dotted",
        "lints.workspace = true\n\n[package]\nname = \"dotted\"\n",
    );

    check_workspace(workspace.path()).unwrap();
}

#[test]
fn member_without_a_lints_table_is_reported() {
    let workspace = Workspace::new();
    workspace.member("bare", "[package]\nname = \"bare\"\n");

    let violations = check_workspace(workspace.path()).unwrap_err();

    assert_eq!(
        failing_members(&violations),
        [Path::new("crates/bare/Cargo.toml")]
    );
}

#[test]
fn member_with_only_its_own_lint_levels_is_reported() {
    let workspace = Workspace::new();
    workspace.member(
        "local",
        "[package]\nname = \"local\"\n\n[lints.rust]\nunsafe_code = \"forbid\"\n",
    );

    let violations = check_workspace(workspace.path()).unwrap_err();

    assert_eq!(
        failing_members(&violations),
        [Path::new("crates/local/Cargo.toml")]
    );
}

#[test]
fn every_failing_member_is_named_and_inheriting_members_are_not() {
    let workspace = Workspace::new();
    workspace.member("inheriting", INHERITING);
    workspace.member("first", "[package]\nname = \"first\"\n");
    workspace.member("second", "[package]\nname = \"second\"\n");

    let violations = check_workspace(workspace.path()).unwrap_err();

    assert_eq!(
        failing_members(&violations),
        [
            Path::new("crates/first/Cargo.toml"),
            Path::new("crates/second/Cargo.toml")
        ]
    );
}

#[test]
fn literal_member_path_is_checked() {
    let workspace = Workspace::new();
    workspace.write("Cargo.toml", "[workspace]\nmembers = [\"tools/one\"]\n");
    workspace.write("tools/one/Cargo.toml", "[package]\nname = \"one\"\n");

    let violations = check_workspace(workspace.path()).unwrap_err();

    assert_eq!(
        failing_members(&violations),
        [Path::new("tools/one/Cargo.toml")]
    );
}

#[test]
fn directory_without_a_manifest_is_not_a_member() {
    let workspace = Workspace::new();
    workspace.write("crates/notes/README.md", "no manifest\n");

    check_workspace(workspace.path()).unwrap();
}

#[test]
fn member_pattern_other_than_a_directory_glob_is_refused() {
    let workspace = Workspace::new();
    workspace.write(
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/*/inner\"]\n",
    );

    let violations = check_workspace(workspace.path()).unwrap_err();

    assert!(
        matches!(&violations[..], [Violation::UnsupportedMemberPattern(pattern)] if pattern == "crates/*/inner"),
        "{violations:?}"
    );
}

#[test]
fn malformed_member_manifest_is_reported_with_its_path() {
    let workspace = Workspace::new();
    workspace.member("broken", "[package\nname = \"broken\"\n");

    let violations = check_workspace(workspace.path()).unwrap_err();

    let expected = workspace.path().join("crates/broken/Cargo.toml");
    assert!(
        matches!(&violations[..], [Violation::Malformed { path, .. }] if *path == expected),
        "{violations:?}"
    );
}

#[test]
fn glob_over_a_missing_directory_has_no_members() {
    let workspace = Workspace::new();

    check_workspace(workspace.path()).unwrap();
}

#[test]
fn unreadable_root_manifest_is_reported_with_its_path() {
    let workspace = Workspace::new();
    let manifest: PathBuf = workspace.path().join("Cargo.toml");
    std::fs::remove_file(&manifest).unwrap();

    let violations = check_workspace(workspace.path()).unwrap_err();

    assert!(
        matches!(&violations[0], Violation::Unreadable { path, .. } if *path == manifest),
        "{violations:?}"
    );
}
