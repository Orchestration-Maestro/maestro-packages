//! Prepares the chat example's browser exports for a nextest run.
//!
//! Nextest runs this setup once, before the browser tests. It compiles `tests/program` to
//! WebAssembly, binds it with the `wasm-bindgen` command line that matches the locked library
//! and exports the bound package's location as `MAESTRO_BROWSER_EXPORTS`.

use std::{
    ffi::OsString,
    fs, io,
    path::Path,
    process::{Command, ExitCode},
};

/// The example that holds the browser exports.
const PROGRAM: &str = "browser_exports";

/// Run a command to completion, naming it in the log and failing on a nonzero status.
fn run(command: &mut Command) -> io::Result<String> {
    println!(
        "COMMAND {} {}",
        command.get_program().to_string_lossy(),
        command
            .get_args()
            .map(|argument| argument.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ")
    );
    let output = command.stderr(std::process::Stdio::inherit()).output()?;
    if output.status.success() {
        String::from_utf8(output.stdout).map_err(io::Error::other)
    } else {
        Err(io::Error::other(format!(
            "{} failed: {}",
            command.get_program().to_string_lossy(),
            output.status
        )))
    }
}

/// Require the `wasm-bindgen` command line to be the version the lock pins for the library.
fn check_binding_version(locked: &str) -> io::Result<()> {
    let expected = format!("wasm-bindgen {locked}");
    let found = run(Command::new("wasm-bindgen").arg("--version"))?;
    if found.trim() == expected {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "the wasm-bindgen command line must match Cargo.lock: expected {expected}, found {}",
            found.trim()
        )))
    }
}

/// Compile the browser exports, bind them into a fresh package and export its location.
fn prepare(cargo: &Path, checkout: &Path) -> io::Result<()> {
    let metadata = maestro_tooling::locked_dependencies(cargo, checkout)?;
    let locked = metadata
        .packages
        .iter()
        .find(|package| package.name.as_str() == "wasm-bindgen")
        .ok_or_else(|| io::Error::other("Cargo.lock holds no wasm-bindgen"))?;
    check_binding_version(&locked.version.to_string())?;
    run(Command::new(cargo)
        .current_dir(checkout)
        .args([
            "rustc",
            "--locked",
            "-p",
            "maestro-web-example",
            "--example",
            PROGRAM,
        ])
        .args(["--target", "wasm32-unknown-unknown", "--", "--cfg", "test"]))?;
    let target = metadata.target_directory.into_std_path_buf();
    let package = target.join("browser-exports");
    if package.exists() {
        fs::remove_dir_all(&package)?;
    }
    run(Command::new("wasm-bindgen")
        .arg(target.join(format!(
            "wasm32-unknown-unknown/debug/examples/{PROGRAM}.wasm"
        )))
        .args([
            "--target",
            "web",
            "--split-linked-modules",
            "--out-name",
            "sandbox",
        ])
        .arg("--out-dir")
        .arg(&package))?;
    let mut entry = OsString::from("MAESTRO_BROWSER_EXPORTS=");
    entry.push(&package);
    maestro_tooling::nextest_setup::export(&entry)
}

fn main() -> ExitCode {
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    match prepare(Path::new(env!("CARGO")), &checkout) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
