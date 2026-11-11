//! Languages the repository does not track.

use maestro_packages_conventions::{Violation, check_workspace};

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
        "lib/module.mjs",
        "lib/module.cjs",
        "ui/component.jsx",
        "ui/component.tsx",
        "src/module.mts",
        "src/module.cts",
        "types/stubs.pyi",
        "win/tool.pyw",
        "bin/run.bash",
        "bin/run.zsh",
        "setup.ps1",
    ];
    for script in scripts {
        workspace.track(script, "\n");
    }

    let violations = check_workspace(workspace.path()).unwrap_err();

    let mut reported: Vec<&str> = violations
        .iter()
        .map(|violation| match violation {
            Violation::TrackedScript(path) => path.as_str(),
            other => panic!("unexpected violation {other:?}"),
        })
        .collect();
    reported.sort_unstable();
    let mut expected = scripts.to_vec();
    expected.sort_unstable();
    assert_eq!(reported, expected);
}

#[test]
fn rust_and_data_files_are_accepted() {
    let workspace = Workspace::new();
    for file in [
        "src/lib.rs",
        "README.md",
        "package.json",
        "config.yaml",
        "notes.txt",
        "types.d.rs",
        "Justfile",
    ] {
        workspace.track(file, "\n");
    }

    check_workspace(workspace.path()).unwrap();
}

#[test]
fn untracked_and_ignored_scripts_are_not_reported() {
    let workspace = Workspace::new();
    workspace.track(".gitignore", "ignored/\n");
    workspace.write("ignored/build.sh", "\n");
    workspace.write("scratch.py", "\n");

    check_workspace(workspace.path()).unwrap();
}

#[test]
fn a_directory_outside_a_repository_is_an_error() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    std::fs::write(
        scratch.path().join("Cargo.toml"),
        "[workspace]\nmembers = []\n",
    )
    .unwrap();

    let violations = check_workspace(scratch.path()).unwrap_err();

    assert!(
        matches!(&violations[..], [Violation::Unreadable { path, .. }] if path == scratch.path()),
        "{violations:?}"
    );
}
