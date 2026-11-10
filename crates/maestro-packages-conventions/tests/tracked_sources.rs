//! Languages the repository does not track.

use maestro_packages_conventions::check_workspace;

use crate::support::Workspace;

#[test]
fn tracked_python_javascript_typescript_and_shell_files_are_reported() {
    let workspace = Workspace::new();
    let scripts = [
        "tool.py",
        "scripts/build.js",
        "examples/extension.ts",
        "crates/inner/run.sh",
        "UPPER.PY",
    ];
    for script in scripts {
        workspace.track(script, "\n");
    }

    let error = check_workspace(workspace.path()).unwrap_err();

    for script in scripts {
        assert!(error.contains(script), "{script} missing from {error}");
    }
}

#[test]
fn rust_and_data_files_are_accepted() {
    let workspace = Workspace::new();
    for file in [
        "src/lib.rs",
        "README.md",
        "package.json",
        "config.yaml",
        "stubs.pyi",
        "component.tsx",
        "Justfile",
    ] {
        workspace.track(file, "\n");
    }

    assert_eq!(check_workspace(workspace.path()), Ok(()));
}

#[test]
fn untracked_and_ignored_scripts_are_not_reported() {
    let workspace = Workspace::new();
    workspace.track(".gitignore", "ignored/\n");
    workspace.write("ignored/build.sh", "\n");
    workspace.write("scratch.py", "\n");

    assert_eq!(check_workspace(workspace.path()), Ok(()));
}

#[test]
fn a_directory_outside_a_repository_is_an_error() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    std::fs::write(
        scratch.path().join("Cargo.toml"),
        "[workspace]\nmembers = []\n",
    )
    .unwrap();

    let error = check_workspace(scratch.path()).unwrap_err();

    assert!(error.contains("tracked sources"), "{error}");
}
