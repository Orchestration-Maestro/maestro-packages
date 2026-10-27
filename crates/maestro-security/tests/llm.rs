//! Model calls exercised over a loopback HTTP bridge.

mod support;

use support::{TestResult, bridge, captured};

/// An HTTP error remains an error when its byte cap splits a Unicode character.
#[test]
fn unicode_http_error_does_not_panic() -> TestResult {
    let prefix = "a".repeat(499);
    let (client, server) = bridge(400, &format!("{prefix}émore"))?;
    let result = client.chat("fixture-model", "system", "user", 16);
    captured(server)?;
    assert_eq!(result.err().as_deref(), Some(format!("bridge http 400: {prefix}").as_str()));
    Ok(())
}
