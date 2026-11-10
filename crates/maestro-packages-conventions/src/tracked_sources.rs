//! Git tracks only Rust sources: no Python, JavaScript, TypeScript or shell file.

use std::io;
use std::path::Path;
use std::process::Command;

use crate::violation::Violation;

/// Extensions of the source languages the repository does not track.
const FORBIDDEN: [&str; 15] = [
    "js", "mjs", "cjs", "jsx", "ts", "mts", "cts", "tsx", "py", "pyi", "pyw", "sh", "bash", "zsh",
    "ps1",
];

/// Report each file git tracks in the repository at `root` that is in a language other than Rust.
pub(crate) fn check(root: &Path) -> Result<Vec<Violation>, Violation> {
    Ok(tracked_files(root)?
        .into_iter()
        .filter(|path| is_script(path))
        .map(Violation::TrackedScript)
        .collect())
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

/// The paths in the index of the repository at `root`, which is what a commit would track.
fn tracked_files(root: &Path) -> Result<Vec<String>, Violation> {
    let unreadable = |source| Violation::Unreadable {
        path: root.to_path_buf(),
        source,
    };
    // `root` alone picks the repository: a commit hook exports variables that would redirect git.
    let output = Command::new("git")
        .current_dir(root)
        .args(["ls-files", "-z"])
        .env_remove("GIT_DIR")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_WORK_TREE")
        .output()
        .map_err(unreadable)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(unreadable(io::Error::other(stderr.trim().to_owned())));
    }
    output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| {
            String::from_utf8(path.to_vec()).map_err(|_| {
                unreadable(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "a tracked path is not UTF-8",
                ))
            })
        })
        .collect()
}
