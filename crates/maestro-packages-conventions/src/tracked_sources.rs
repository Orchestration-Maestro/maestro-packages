//! Git tracks only Rust sources: no Python, JavaScript, TypeScript or shell file.

use std::path::Path;
use std::process::Command;

/// Extensions of the source languages the repository does not track.
const FORBIDDEN: [&str; 4] = ["py", "js", "ts", "sh"];

/// Fail when git tracks a file of a language other than Rust in the repository at `root`.
pub(crate) fn check(root: &Path) -> Result<(), String> {
    let scripts: Vec<String> = tracked_files(root)?
        .into_iter()
        .filter(|path| is_script(path))
        .collect();
    if scripts.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "tracked sources: only Rust is tracked, remove: {}",
            scripts.join(", ")
        ))
    }
}

/// Whether `path` ends in the extension of a language the repository does not track.
fn is_script(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            FORBIDDEN
                .iter()
                .any(|forbidden| forbidden.eq_ignore_ascii_case(extension))
        })
}

/// The paths in the repository's index, which is what a commit would track.
fn tracked_files(root: &Path) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(["ls-files", "-z"])
        .env_remove("GIT_DIR")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_WORK_TREE")
        .output()
        .map_err(|error| format!("tracked sources: git: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "tracked sources: git ls-files in {}: {}",
            root.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| {
            String::from_utf8(path.to_vec())
                .map_err(|_| "tracked sources: a tracked path is not UTF-8".to_owned())
        })
        .collect()
}
