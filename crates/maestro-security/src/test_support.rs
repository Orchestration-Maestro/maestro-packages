//! Isolated state and assessment fixtures for capability tests.
use std::path::Path;

use maestro_test_directory::Scratch;

/// Run with a fresh state directory, never the developer's retained evidence.
pub(crate) fn isolated(body: impl FnOnce(&Path)) {
    let home = Scratch::new().expect("fresh state directory");
    crate::evidence::with_home(home.path(), || body(home.path()));
}

/// Minimal completed assessment accepted by the retained report interface.
pub(crate) fn assessment() -> crate::report::Assessment {
    serde_json::from_value(serde_json::json!({
        "schema": crate::report::SCHEMA, "run_id": "run-test", "agent": "security",
        "target": "example.test", "model": "resolved", "phases": [], "findings": [{
            "id": "F-001", "fingerprint": "example-access", "title": "Access candidate",
            "class": "access", "verdict": "needs_validation", "severity": "low",
            "confidence": 0.5, "evidence_level": "LIKELY", "escalation": "none",
            "evidence": {"command": "probe", "output": "response", "id": "ev-0001"},
            "impact": "potential read access", "fix": "check authorization", "target": "example.test"
        }],
        "unknown": [], "refuted": [], "summary": "One candidate requires validation."
    }))
    .expect("assessment fixture")
}

/// Make an accepted fixture stream blocking before the fixture reads from it.
///
/// Accepted sockets keep the nonblocking mode of a nonblocking listener on
/// macOS and Windows, so the first read can fail with `WouldBlock`.
///
/// # Errors
/// Returns an error if the stream's mode cannot be changed or read, or if the
/// stream is still nonblocking afterwards.
pub(crate) fn blocking(stream: &std::net::TcpStream) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    #[cfg(unix)]
    if socket2::SockRef::from(stream).nonblocking()? {
        return Err(std::io::Error::other(
            "accepted fixture stream must be blocking",
        ));
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use std::net::{TcpListener, TcpStream};

    /// A stream that arrives nonblocking is made blocking before it is read.
    #[test]
    fn accepted_fixture_stream_is_made_blocking() -> std::io::Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let stream = TcpStream::connect(listener.local_addr()?)?;
        stream.set_nonblocking(true)?;
        assert!(socket2::SockRef::from(&stream).nonblocking()?);

        super::blocking(&stream)?;

        assert!(!socket2::SockRef::from(&stream).nonblocking()?);
        Ok(())
    }
}
