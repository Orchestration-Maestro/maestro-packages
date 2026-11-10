//! Every workspace member inherits the workspace lint table with `[lints] workspace = true`.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::violation::Violation;

/// The root manifest of the workspace.
#[derive(Deserialize)]
struct Root {
    /// The `[workspace]` table.
    workspace: Workspace,
}

/// The part of `[workspace]` that names the members.
#[derive(Deserialize)]
struct Workspace {
    /// Member directories: a literal path, or a directory followed by `/*`.
    #[serde(default)]
    members: Vec<String>,
}

/// The part of a member manifest that decides lint inheritance.
#[derive(Deserialize)]
struct Member {
    /// The `[lints]` table; absent when the member sets none.
    #[serde(default)]
    lints: Lints,
}

/// A member's `[lints]` table, of which only the inheritance switch matters.
#[derive(Default, Deserialize)]
struct Lints {
    /// `workspace = true` inherits the workspace table.
    #[serde(default)]
    workspace: bool,
}

/// Report each member of the workspace at `root` that does not inherit the workspace lints.
pub(crate) fn check(root: &Path) -> Result<Vec<Violation>, Violation> {
    let workspace = read::<Root>(&root.join("Cargo.toml"))?.workspace;
    let mut violations = Vec::new();
    for manifest in manifests(root, &workspace.members)? {
        if !read::<Member>(&manifest)?.lints.workspace {
            let relative = manifest.strip_prefix(root).unwrap_or(&manifest);
            violations.push(Violation::MemberWithoutWorkspaceLints(
                relative.to_path_buf(),
            ));
        }
    }
    Ok(violations)
}

/// Parse the manifest at `path`.
fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, Violation> {
    let text = fs::read_to_string(path).map_err(|source| unreadable(path, source))?;
    toml::from_str(&text).map_err(|source| Violation::Malformed {
        path: path.to_path_buf(),
        source,
    })
}

/// The failure for a path that could not be read.
fn unreadable(path: &Path, source: std::io::Error) -> Violation {
    Violation::Unreadable {
        path: path.to_path_buf(),
        source,
    }
}

/// The member manifests, in name order, that the `members` patterns name.
fn manifests(root: &Path, patterns: &[String]) -> Result<Vec<PathBuf>, Violation> {
    let mut manifests = Vec::new();
    for pattern in patterns {
        let (path, is_directory_glob) = pattern
            .strip_suffix("/*")
            .map_or((pattern.as_str(), false), |parent| (parent, true));
        if has_wildcard(path) {
            return Err(Violation::UnsupportedMemberPattern(pattern.clone()));
        }
        if is_directory_glob {
            manifests.extend(children(root, path)?);
        } else {
            manifests.push(root.join(path).join("Cargo.toml"));
        }
    }
    Ok(manifests)
}

/// The manifests of the directories directly below `parent`, in name order.
fn children(root: &Path, parent: &str) -> Result<Vec<PathBuf>, Violation> {
    let directory = root.join(parent);
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(unreadable(&directory, source)),
    };
    let mut manifests = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| unreadable(&directory, source))?;
        let manifest = entry.path().join("Cargo.toml");
        if manifest.is_file() {
            manifests.push(manifest);
        }
    }
    manifests.sort();
    Ok(manifests)
}

/// Whether `pattern` holds a glob character this check does not expand.
fn has_wildcard(pattern: &str) -> bool {
    pattern.contains(['*', '?', '['])
}
