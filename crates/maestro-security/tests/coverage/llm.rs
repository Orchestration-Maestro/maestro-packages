//! Model calls exercised over a loopback HTTP bridge.

use super::support::{TestResult, bridge, captured};

/// An HTTP error remains an error when its byte cap splits a Unicode character.
#[test]
fn unicode_http_error_does_not_panic() -> TestResult {
    let prefix = "a".repeat(499);
    let (client, server) = bridge(400, &format!("{prefix}émore"))?;
    let result = client.chat("fixture-model", "system", "user", 16);
    captured(server)?;
    assert_eq!(
        result.err().as_deref(),
        Some(format!("bridge http 400: {prefix}").as_str())
    );
    Ok(())
}

/// Catalog order survives filtering entries that do not carry string ids.
#[test]
fn baseline_catalog_preserves_order_and_authorizes() -> TestResult {
    let (client, server) = bridge(
        200,
        r#"{"data":[{"id":"second"},{"id":9},{},{"id":"first"}]}"#,
    )?;
    assert_eq!(client.available_models()?, ["second", "first"]);
    let request = captured(server)?;
    assert!(request.starts_with("GET /v1/models HTTP/1.1\r\n"));
    assert!(request.contains("\r\nauthorization: Bearer fixture-key\r\n"));
    Ok(())
}

/// Invalid catalogs fail instead of supplying an unusable model selection.
#[test]
fn baseline_catalog_rejects_errors_and_empty_models() -> TestResult {
    for (status, body, error) in [
        (401, "denied", "bridge http 401 on /models"),
        (200, "{", "bridge json:"),
        (200, r#"{"data":[]}"#, "returned no models"),
        (200, r#"{"data":[{"id":null}]}"#, "returned no models"),
        (200, r#"{"data":{}}"#, "returned no models"),
        (200, "{}", "returned no models"),
    ] {
        let (client, server) = bridge(status, body)?;
        let result = client.available_models();
        captured(server)?;
        assert!(
            result.is_err_and(|message| message.contains(error)),
            "{status}: {body}"
        );
    }
    Ok(())
}

/// A completion sends the caller's inputs and returns untrimmed content and usage.
#[test]
fn baseline_chat_sends_inputs_and_returns_content_and_tokens() -> TestResult {
    let (client, server) = bridge(
        200,
        r#"{"choices":[{"message":{"content":" answer\n"}}],"usage":{"completion_tokens":17}}"#,
    )?;
    let reply = client.chat("chosen", "system text", "user text", 123)?;
    assert_eq!(reply.content, " answer\n");
    assert_eq!(reply.tokens_out, 17);
    let request = captured(server)?;
    assert!(request.starts_with("POST /v1/chat/completions HTTP/1.1\r\n"));
    assert!(request.contains("\r\nauthorization: Bearer fixture-key\r\n"));
    assert!(request.contains("\r\ncontent-type: application/json\r\n"));
    let (_, body) = request
        .split_once("\r\n\r\n")
        .ok_or("missing request body")?;
    let body: serde_json::Value = serde_json::from_str(body)?;
    assert_eq!(body["model"], "chosen");
    assert_eq!(body["max_tokens"], 123);
    assert_eq!(
        body["messages"],
        serde_json::json!([
            {"role":"system", "content":"system text"},
            {"role":"user", "content":"user text"}
        ])
    );
    Ok(())
}

/// Missing or non-integer usage does not discard a usable answer.
#[test]
fn baseline_chat_defaults_unreported_usage_to_zero() -> TestResult {
    for usage in [
        serde_json::Value::Null,
        serde_json::json!({"completion_tokens":-1}),
    ] {
        let body = serde_json::json!({"choices":[{"message":{"content":"ok"}}], "usage":usage});
        let (client, server) = bridge(200, &body.to_string())?;
        let reply = client.chat("chosen", "", "question", 8)?;
        captured(server)?;
        assert_eq!(reply.content, "ok");
        assert_eq!(reply.tokens_out, 0);
    }
    Ok(())
}

/// Malformed or blank answers and HTTP failures are returned to the caller.
#[test]
fn baseline_chat_rejects_unusable_responses() -> TestResult {
    for (status, body, expected) in [
        (403, "denied", "bridge http 403: denied"),
        (200, "{", "bridge json:"),
        (200, "{}", "bridge returned no message content"),
        (
            200,
            r#"{"choices":[{"message":{"content":42}}]}"#,
            "no message content",
        ),
        (
            200,
            r#"{"choices":[{"message":{"content":" \n\t"}}]}"#,
            "no message content",
        ),
    ] {
        let (client, server) = bridge(status, body)?;
        let result = client.chat("chosen", "", "question", 8);
        captured(server)?;
        assert!(
            result.is_err_and(|message| message.contains(expected)),
            "{status}: {body}"
        );
    }
    let (client, server) = bridge(400, &"x".repeat(501))?;
    let result = client.chat("chosen", "", "question", 8);
    captured(server)?;
    assert_eq!(
        result.err(),
        Some(format!("bridge http 400: {}", "x".repeat(500)))
    );
    Ok(())
}

/// Transport setup failures remain recoverable for both model operations.
#[test]
fn baseline_calls_propagate_transport_errors() {
    let client = maestro_security::Client::local().with_base_url("unsupported://fixture");
    assert!(
        client
            .available_models()
            .is_err_and(|error| error.contains("unsupported url"))
    );
    assert!(
        client
            .chat("model", "", "", 1)
            .is_err_and(|error| error.contains("unsupported url"))
    );
}
