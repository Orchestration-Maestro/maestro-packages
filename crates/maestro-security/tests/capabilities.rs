//! Behavioural coverage through the public capability APIs and HTTP boundaries.

#[path = "capabilities/disclose.rs"]
mod disclose;
#[path = "capabilities/exploit.rs"]
mod exploit;
#[path = "capabilities/remediate.rs"]
mod remediate;
#[path = "capabilities/reverse.rs"]
mod reverse;
#[path = "capabilities/sourcehunt.rs"]
mod sourcehunt;
#[path = "capabilities/support.rs"]
mod support;

#[path = "capabilities/audit.rs"]
mod audit;
#[path = "capabilities/bench.rs"]
mod bench;
#[path = "capabilities/coverage.rs"]
mod coverage;
#[path = "capabilities/doctor.rs"]
mod doctor;
#[path = "support/handler.rs"]
mod handler_support;
#[path = "capabilities/report.rs"]
mod report;

#[cfg(test)]
mod tests {
    use super::*;
    use maestro_test_directory::Scratch;
    use maestro_test_process::TestProcess;

    /// No HTTP exchange is needed to exercise directory ownership.
    fn no_requests(_: &str, _: &serde_json::Value) -> (u16, String) {
        (404, String::new())
    }

    /// Confirm that the fixture captured the complete fragmented request.
    fn complete_body(path: &str, body: &serde_json::Value) -> (u16, String) {
        assert_eq!(path, "/capture");
        assert_eq!(body["payload"], "x".repeat(65_536));
        (200, "captured".into())
    }

    /// A nonblocking listener still serves a body arriving across many blocking reads.
    #[test]
    fn accepted_stream_captures_fragmented_body() -> Result<(), Box<dyn std::error::Error>> {
        use std::io::{Read, Write};

        if !handler_support::isolated(
            "tests::accepted_stream_captures_fragmented_body",
            complete_body,
        )? {
            return Ok(());
        }
        let url = std::env::var("MAESTRO_SECURITY_BRIDGE")?;
        let mut client = std::net::TcpStream::connect(
            url.strip_prefix("http://").ok_or("missing fixture URL")?,
        )?;
        let body = serde_json::json!({"payload": "x".repeat(65_536)}).to_string();
        write!(
            client,
            "POST /capture HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )?;
        for byte in body.as_bytes() {
            client.write_all(std::slice::from_ref(byte))?;
            std::thread::yield_now();
        }
        let mut response = String::new();
        client.read_to_string(&mut response)?;
        assert!(response.starts_with("HTTP/1.1 200 "));
        assert!(response.ends_with("\r\n\r\ncaptured"));
        Ok(())
    }

    /// Qualified test identities never become directory leaves, and homes are removed.
    #[test]
    fn isolated_home_is_portable_and_removed() -> Result<(), Box<dyn std::error::Error>> {
        let name = "tests::isolated_home_is_portable_and_removed";
        if std::env::var_os("DIRECTORY_TEST_CHILD").is_some() {
            if handler_support::isolated(name, no_requests)? {
                let home = std::path::PathBuf::from(
                    std::env::var_os("MAESTRO_SECURITY_HOME").ok_or("missing home")?,
                );
                let leaf = home
                    .file_name()
                    .ok_or("missing leaf")?
                    .to_str()
                    .ok_or("non-UTF-8 leaf")?;
                assert!(
                    leaf.chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-'),
                    "nonportable directory: {leaf}"
                );
                assert_eq!(std::env::var("SECURITY_TEST_CHILD")?, name);
                std::fs::write(home.join("child-ran"), "evidence")?;
            }
            return Ok(());
        }
        let root = Scratch::new()?;
        let status = TestProcess::current(name)?
            .env("DIRECTORY_TEST_CHILD", "1")
            .temp_dir(root.path())
            .status()?;
        assert!(status.success());
        assert_eq!(std::fs::read_dir(root.path())?.count(), 0);
        Ok(())
    }

    /// An invalid child selection releases the owned home before returning its error.
    #[test]
    fn failed_child_selection_removes_home() -> Result<(), Box<dyn std::error::Error>> {
        let name = "tests::failed_child_selection_removes_home";
        if std::env::var_os("DIRECTORY_TEST_CHILD").is_some() {
            assert!(handler_support::isolated("missing::test", no_requests).is_err());
            return Ok(());
        }
        let root = Scratch::new()?;
        let status = TestProcess::current(name)?
            .env("DIRECTORY_TEST_CHILD", "1")
            .temp_dir(root.path())
            .status()?;
        assert!(status.success());
        assert_eq!(std::fs::read_dir(root.path())?.count(), 0);
        Ok(())
    }
}
