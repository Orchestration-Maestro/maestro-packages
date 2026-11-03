//! Behavioural coverage for model calls, role resolution and task routing.

#[path = "coverage/llm.rs"]
mod llm;
#[path = "coverage/models.rs"]
mod models;
#[path = "coverage/router.rs"]
mod router;
#[cfg(test)]
pub mod support;

/// A stream that arrives nonblocking, as accepted sockets do on BSD-derived
/// systems, must be made blocking before the fixture reads from it.
#[test]
#[cfg(unix)]
fn inherited_nonblocking_stream_is_made_blocking() -> support::TestResult {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let stream = std::net::TcpStream::connect(listener.local_addr()?)?;
    stream.set_nonblocking(true)?;
    assert!(socket2::SockRef::from(&stream).nonblocking()?);

    support::blocking(&stream)?;

    assert!(!socket2::SockRef::from(&stream).nonblocking()?);
    Ok(())
}
