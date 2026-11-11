//! Prepares the Freedoom IWAD that the DOOM overlay's real-engine tests play.
//!
//! Nextest runs this setup once, before those tests. It downloads the pinned release archive,
//! refuses it unless its SHA-256 matches, extracts Phase 1's IWAD under the target directory and
//! exports the file's location as `MAESTRO_FREEDOOM_WAD`. Nothing from the archive is committed.

/// The archive checks, which the host tests run as well.
#[cfg(not(target_arch = "wasm32"))]
#[path = "host/freedoom_archive.rs"]
mod freedoom_archive;

/// The download, the checks and the export, for hosts that can run them.
#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::freedoom_archive::checked_iwad;
    use std::{ffi::OsString, fs, path::Path, process::ExitCode};

    /// The Freedoom 0.13.0 release archive.
    const ARCHIVE_URL: &str =
        "https://github.com/freedoom/freedoom/releases/download/v0.13.0/freedoom-0.13.0.zip";
    /// The archive's SHA-256, checked before anything is extracted.
    const ARCHIVE_SHA256: &str = "3f9b264f3e3ce503b4fb7f6bdcb1f419d93c7b546f4df3e874dd878db9688f59";
    /// Phase 1's IWAD inside the archive.
    const IWAD: &str = "freedoom-0.13.0/freedoom1.wad";
    /// Redirects followed before the download is given up.
    const REDIRECTS: usize = 3;

    /// Downloads one URL, following the release host's redirect to its signed asset address.
    fn download(url: &str) -> Result<Vec<u8>, String> {
        let mut url = url.to_owned();
        for _ in 0..=REDIRECTS {
            let response = maestro_http::request("GET", &url, &[], &[])?;
            match (response.status, response.location) {
                (200, _) => return Ok(response.body),
                (301 | 302 | 303 | 307 | 308, Some(next)) => url = next,
                (status, _) => return Err(format!("{url} answered HTTP {status}")),
            }
        }
        Err(format!("{url} redirected more than {REDIRECTS} times"))
    }

    /// Downloads, checks and extracts the IWAD, then exports where it is.
    fn prepare(cargo: &Path, checkout: &Path) -> Result<(), String> {
        let target = maestro_tooling::locked_dependencies(cargo, checkout)
            .map_err(|e| e.to_string())?
            .target_directory
            .into_std_path_buf()
            .join("freedoom");
        let iwad = checked_iwad(&download(ARCHIVE_URL)?, ARCHIVE_SHA256, IWAD)?;
        fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        let path = target.join("freedoom1.wad");
        fs::write(&path, iwad).map_err(|e| e.to_string())?;
        let mut entry = OsString::from("MAESTRO_FREEDOOM_WAD=");
        entry.push(&path);
        maestro_tooling::nextest_setup::export(&entry).map_err(|e| e.to_string())
    }

    /// Runs the setup for this checkout.
    pub(super) fn main() -> ExitCode {
        let checkout = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        match prepare(Path::new(env!("CARGO")), &checkout) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        }
    }
}

/// Runs the setup; a WebAssembly build has nothing to prepare.
#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    native::main()
}

/// A WebAssembly build of the examples still needs an entry point.
#[cfg(target_arch = "wasm32")]
fn main() {}
