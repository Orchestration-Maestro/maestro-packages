//! Stand-in for the SSH client the example launches, installed on the checked child's path.

use std::process::ExitCode;

/// Reports the failure the test controls, or runs the remote command in the `remote` directory
/// beside this program.
#[cfg(unix)]
fn main() -> ExitCode {
    use std::os::unix::process::CommandExt as _;

    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [separator, remote, command] = arguments.as_slice() else {
        return refuse("expects -- <destination> <command>");
    };
    if separator != "--" {
        return refuse("expects -- <destination> <command>");
    }
    if remote == "user@broken" {
        eprint!("controlled connection failure");
        return ExitCode::from(37);
    }
    if command.starts_with("cat > ") && command.contains("early-write.txt") {
        eprint!("controlled remote write rejection");
        return ExitCode::from(38);
    }
    if remote != "user@controlled" {
        return refuse("received an unexpected destination");
    }
    let directory = match std::env::current_exe() {
        Ok(program) => program.with_file_name("remote"),
        Err(error) => return refuse(&format!("cannot locate its own program: {error}")),
    };
    if let Err(error) = std::env::set_current_dir(&directory) {
        return refuse(&format!("cannot enter {}: {error}", directory.display()));
    }
    let error = std::process::Command::new("bash")
        .arg("-c")
        .arg(command)
        .exec();
    refuse(&format!("cannot start bash: {error}"))
}

/// Reports a rejected invocation without running anything.
#[cfg(unix)]
fn refuse(message: &str) -> ExitCode {
    eprint!("ssh stand-in {message}");
    ExitCode::from(2)
}

/// Does nothing where the controlled remote shell does not exist.
#[cfg(not(unix))]
fn main() {}
