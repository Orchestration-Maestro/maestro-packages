//! A disposable git repository that holds a workspace.

use std::fs;
use std::path::Path;
use std::process::Command;

use maestro_test_directory::Scratch;

/// A git repository whose root manifest lists `crates/*` as members.
pub(crate) struct Workspace {
    /// Owner of the repository directory.
    scratch: Scratch,
}

impl Workspace {
    /// An initialised repository with a root manifest and no member.
    pub(crate) fn new() -> Self {
        let scratch = Scratch::new().unwrap();
        let workspace = Self { scratch };
        git(workspace.path(), &["init", "--quiet"]);
        workspace.write("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n");
        workspace
    }

    /// The repository root.
    pub(crate) fn path(&self) -> &Path {
        self.scratch.path()
    }

    /// Write `contents` to `relative`, creating its directories, without tracking it.
    pub(crate) fn write(&self, relative: &str, contents: &str) {
        let path = self.path().join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    /// Write `contents` to `relative` and stage it.
    pub(crate) fn track(&self, relative: &str, contents: &str) {
        self.write(relative, contents);
        git(self.path(), &["add", "--", relative]);
    }

    /// Add the member `crates/<name>` with the manifest `manifest`.
    pub(crate) fn member(&self, name: &str, manifest: &str) {
        self.write(&format!("crates/{name}/Cargo.toml"), manifest);
    }
}

/// Run git in `directory` without the repository variables a commit hook exports.
fn git(directory: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .current_dir(directory)
        .args(arguments)
        .env_remove("GIT_DIR")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_WORK_TREE")
        .status()
        .unwrap();
    assert!(status.success(), "git {arguments:?} failed");
}
