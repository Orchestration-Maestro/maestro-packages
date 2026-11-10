//! Every workspace member inherits the workspace lint table with `[lints] workspace = true`.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::Deserialize;

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

/// Fail unless every member of the workspace at `root` inherits the workspace lints.
pub(crate) fn check(root: &Path) -> Result<(), String> {
    let members: Workspace = read::<Root>(&root.join("Cargo.toml"))?.workspace;
    let mut failures = Vec::new();
    for manifest in manifests(root, &members.members)? {
        if !read::<Member>(&manifest)?.lints.workspace {
            let relative = manifest.strip_prefix(root).unwrap_or(&manifest);
            failures.push(relative.display().to_string());
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "workspace lints: members must set `[lints] workspace = true`: {}",
            failures.join(", ")
        ))
    }
}

/// Parse the manifest at `path`.
fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let text = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
}

/// The member manifests, in name order, that the `members` patterns name.
fn manifests(root: &Path, patterns: &[String]) -> Result<Vec<PathBuf>, String> {
    let mut manifests = Vec::new();
    for pattern in patterns {
        if let Some(parent) = pattern.strip_suffix("/*") {
            manifests.extend(children(root, parent)?);
        } else if has_wildcard(pattern) {
            return Err(unsupported(pattern));
        } else {
            manifests.push(root.join(pattern).join("Cargo.toml"));
        }
    }
    Ok(manifests)
}

/// The manifests of the directories directly below `parent`, in name order.
fn children(root: &Path, parent: &str) -> Result<Vec<PathBuf>, String> {
    if has_wildcard(parent) {
        return Err(unsupported(parent));
    }
    let directory = root.join(parent);
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("{}: {error}", directory.display())),
    };
    let mut manifests = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("{}: {error}", directory.display()))?;
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

/// The failure for a member pattern other than a path or a directory followed by `/*`.
fn unsupported(pattern: &str) -> String {
    format!("workspace lints: member pattern `{pattern}` is not a path or `<directory>/*`")
}
