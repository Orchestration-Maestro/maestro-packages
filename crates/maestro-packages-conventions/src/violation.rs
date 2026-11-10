//! The failures a repository check reports.

use std::error::Error;
use std::fmt;
use std::io;
use std::path::PathBuf;

/// One failed repository check.
#[derive(Debug)]
pub enum Violation {
    /// A member whose manifest, relative to the root, does not set `[lints] workspace = true`.
    MemberWithoutWorkspaceLints(PathBuf),
    /// A `[workspace] members` pattern that is neither a path nor `<directory>/*`.
    UnsupportedMemberPattern(String),
    /// A tracked file, relative to the root, in a language other than Rust.
    TrackedScript(String),
    /// A manifest or repository that could not be read.
    Unreadable {
        /// The manifest, directory or repository root that failed.
        path: PathBuf,
        /// Why reading failed.
        source: io::Error,
    },
    /// A manifest that is not valid TOML or lacks `[workspace]`.
    Malformed {
        /// The manifest that failed.
        path: PathBuf,
        /// Why parsing failed.
        source: toml::de::Error,
    },
}

impl fmt::Display for Violation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MemberWithoutWorkspaceLints(manifest) => write!(
                formatter,
                "member {} must set `[lints] workspace = true`",
                manifest.display()
            ),
            Self::UnsupportedMemberPattern(pattern) => write!(
                formatter,
                "member pattern `{pattern}` is not a path or `<directory>/*`"
            ),
            Self::TrackedScript(path) => {
                write!(
                    formatter,
                    "tracked file {path} is not Rust; remove it or write it in Rust"
                )
            }
            Self::Unreadable { path, .. } => write!(formatter, "cannot read {}", path.display()),
            Self::Malformed { path, .. } => write!(formatter, "cannot parse {}", path.display()),
        }
    }
}

impl Error for Violation {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Unreadable { source, .. } => Some(source),
            Self::Malformed { source, .. } => Some(source),
            Self::MemberWithoutWorkspaceLints(_)
            | Self::UnsupportedMemberPattern(_)
            | Self::TrackedScript(_) => None,
        }
    }
}
