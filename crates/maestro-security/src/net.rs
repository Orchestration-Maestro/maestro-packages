//! Minimal HTTP/1.1 client over WASI sockets for the local bridge and runner.
//!
//! Both peers are loopback HTTP servers with no TLS, so a hand-rolled request
//! writer and read-until-close reader is the whole transport.

use std::io::{Read, Write};
use std::net::TcpStream;

/// A completed HTTP response: status code plus the decoded body.
pub struct Response {
    /// Three-digit status code.
    pub status: u16,
    /// Response body bytes.
    pub body: Vec<u8>,
}

/// Split a `http://host:port/path` URL into host, port and path (with slash).
fn split_url(url: &str) -> Result<(&str, u16, &str), String> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| format!("unsupported url: {url}"))?;
    let (hostport, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().map_err(|e| e.to_string())?),
        None => (hostport, 80),
    };
    Ok((host, port, path))
}

/// Send one HTTP request and read the full response body.
///
/// Blocking by design: both endpoints are loopback and the calls are short.
/// shortcut: blocks the async task thread; move to a spawned worker if the
/// host runtime ever shows contention under concurrent tool calls.
///
/// # Errors
/// Fails on connection failure, a send or read error, or a response without
/// a header terminator.
pub fn request(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Result<Response, String> {
    let (host, port, path) = split_url(url)?;
    let mut stream = TcpStream::connect((host, port))
        .map_err(|e| format!("connect {host}:{port}: {e}"))?;
    let mut head = format!(
        "{method} {path} HTTP/1.1\r\nhost: {host}:{port}\r\ncontent-length: {}\r\nconnection: close\r\n",
        body.len()
    );
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    stream
        .write_all(head.as_bytes())
        .and_then(|()| stream.write_all(body))
        .map_err(|e| format!("send: {e}"))?;

    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|e| format!("read: {e}"))?;
    parse_response(raw)
}

/// Parse a response head + body from raw bytes (read-until-close framing).
fn parse_response(raw: Vec<u8>) -> Result<Response, String> {
    let sep = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("malformed response: no header terminator")?;
    let head = String::from_utf8_lossy(&raw[..sep]).to_string();
    let mut body = raw[sep + 4..].to_vec();
    let mut status = 0u16;
    let mut content_length: Option<usize> = None;
    for line in head.lines() {
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse::<usize>().ok();
            }
        } else if let Some(code) = line.split_whitespace().nth(1) {
            status = code.parse::<u16>().unwrap_or(0);
        }
    }
    if let Some(len) = content_length {
        body.truncate(len);
    }
    Ok(Response { status, body })
}
