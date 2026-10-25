//! Minimal synchronous HTTP/1.1 client for loopback gateways and model upstreams.
//!
//! [`request`] speaks both `http://` (plain TCP) and `https://` (rustls with
//! webpki roots). Responses are read until close and decoded from
//! `content-length` or `transfer-encoding: chunked` framing. Transient
//! failures — transport errors and 429/5xx statuses — are retried with
//! exponential backoff under a caller-settable attempt budget
//! ([`request_with_attempts`]).

use std::borrow::Cow;
use std::fmt::Write as _;
use std::io::{Read, Write};
use std::net::TcpStream;

mod retry;

/// A completed HTTP response: status code plus the decoded body.
#[derive(Debug)]
pub struct Response {
    /// Three-digit status code.
    pub status: u16,
    /// Response body bytes, decoded from any chunked framing.
    pub body: Vec<u8>,
    /// The server's `Retry-After` header in whole seconds, when present and
    /// integer.
    pub retry_after: Option<u64>,
}

/// Send one HTTP request with the default retry budget and read the full
/// response.
///
/// `url` must use `http://` or `https://`. Blocking by design: callers in
/// async runtimes wrap it with a blocking spawn.
///
/// Transport errors and retryable statuses (429, 5xx) are retried with
/// exponential backoff; a `Retry-After` header in seconds is honored. Any
/// other response is returned as-is on the first attempt.
///
/// # Errors
/// Fails on an unsupported scheme, a send or read error after the last
/// attempt, or a response without a header terminator.
pub fn request(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Result<Response, String> {
    request_with_attempts(method, url, headers, body, retry::DEFAULT_ATTEMPTS)
}

/// Send one HTTP request with an explicit attempt budget and read the full
/// response.
///
/// `attempts` is the total number of tries; one means no retry, and values
/// below one are treated as one. A caller that wants a single try or a larger
/// budget (for example five) passes it here. Transient failures — transport
/// errors and 429/5xx statuses — consume an attempt; anything else ends the
/// call immediately with that response.
///
/// # Errors
/// As [`request`], reported for the final attempt.
pub fn request_with_attempts(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: &[u8],
    attempts: u32,
) -> Result<Response, String> {
    let (host, port, path, secure) = split_url(url)?;
    let default_port = if secure { 443 } else { 80 };
    let hostport: Cow<'_, str> = if port == default_port {
        Cow::Borrowed(host)
    } else {
        Cow::Owned(format!("{host}:{port}"))
    };
    let head = request_head(method, &hostport, path, headers, body.len());
    retry::run_with_retry(attempts, || {
        if secure {
            tls_request(host, port, &head, body)
        } else {
            plain_request(host, port, &head, body)
        }
    })
}

/// Split a `http(s)://host:port/path` URL into host, port, path and TLS flag.
fn split_url(url: &str) -> Result<(&str, u16, &str, bool), String> {
    let (rest, secure) = match url.strip_prefix("https://") {
        Some(rest) => (rest, true),
        None => url
            .strip_prefix("http://")
            .map(|rest| (rest, false))
            .ok_or_else(|| format!("unsupported url: {url}"))?,
    };
    let (hostport, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) => (
            h,
            p.parse::<u16>()
                .map_err(|e| format!("bad port in {url}: {e}"))?,
        ),
        None => (hostport, if secure { 443 } else { 80 }),
    };
    Ok((host, port, path, secure))
}

/// Build the request head with the mandatory framing headers.
///
/// `hostport` is the `host:` header value: the host, or `host:port` when the
/// port differs from the scheme default.
fn request_head(
    method: &str,
    hostport: &str,
    path: &str,
    headers: &[(&str, &str)],
    body_len: usize,
) -> String {
    let mut head = format!(
        "{method} {path} HTTP/1.1\r\nhost: {hostport}\r\ncontent-length: {body_len}\r\nconnection: close\r\n",
    );
    for (name, value) in headers {
        let _ = write!(head, "{name}: {value}\r\n");
    }
    head.push_str("\r\n");
    head
}

/// Plain-TCP request: connect, send, read until close.
fn plain_request(host: &str, port: u16, head: &str, body: &[u8]) -> Result<Response, String> {
    let mut stream = TcpStream::connect((host, port))
        .map_err(|e| format!("connect {host}:{port}: {e}"))?;
    send_and_read(&mut stream, head, body, host, port)
}

/// TLS request: rustls session over TCP, then the same send-and-read path.
fn tls_request(host: &str, port: u16, head: &str, body: &[u8]) -> Result<Response, String> {
    use rustls::pki_types::ServerName;
    use rustls::ClientConfig;

    let server_name = ServerName::try_from(host.to_string())
        .map_err(|e| format!("bad tls name {host}: {e}"))?;
    let roots = rustls::RootCertStore::from_iter(
        webpki_roots::TLS_SERVER_ROOTS.iter().cloned(),
    );
    let config = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let session = rustls::ClientConnection::new(std::sync::Arc::new(config), server_name)
        .map_err(|e| format!("tls session {host}: {e}"))?;
    let stream = TcpStream::connect((host, port))
        .map_err(|e| format!("connect {host}:{port}: {e}"))?;
    let mut tls = rustls::StreamOwned::new(session, stream);
    send_and_read(&mut tls, head, body, host, port)
}

/// Write head + body, read the response until close, parse it.
fn send_and_read(
    stream: &mut impl ReadWrite,
    head: &str,
    body: &[u8],
    host: &str,
    port: u16,
) -> Result<Response, String> {
    stream
        .write_all(head.as_bytes())
        .and_then(|()| stream.write_all(body))
        .map_err(|e| format!("send to {host}:{port}: {e}"))?;
    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|e| format!("read from {host}:{port}: {e}"))?;
    parse_response(&raw)
}

/// Read + write over one stream, without naming the concrete type.
trait ReadWrite: Read + Write {}
impl<T: Read + Write + ?Sized> ReadWrite for T {}

/// Parse a response head + body from raw bytes (read-until-close framing).
fn parse_response(raw: &[u8]) -> Result<Response, String> {
    let sep = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("malformed response: no header terminator")?;
    let head = String::from_utf8_lossy(&raw[..sep]).to_string();
    let rest = &raw[sep + 4..];
    let mut body = rest.to_vec();
    let mut status = 0u16;
    let mut content_length: Option<usize> = None;
    let mut chunked = false;
    let mut retry_after: Option<u64> = None;
    for line in head.lines() {
        if let Some((name, value)) = line.split_once(':') {
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim();
            if name == "content-length" {
                content_length = value.parse::<usize>().ok();
            } else if name == "transfer-encoding" && value.eq_ignore_ascii_case("chunked") {
                chunked = true;
            } else if name == "retry-after" {
                retry_after = value.parse::<u64>().ok();
            }
        } else if let Some(code) = line.split_whitespace().nth(1) {
            status = code.parse::<u16>().unwrap_or(0);
        }
    }
    if chunked {
        body = decode_chunked(rest);
    } else if let Some(len) = content_length {
        body.truncate(len);
    }
    Ok(Response {
        status,
        body,
        retry_after,
    })
}

/// Decode an HTTP/1.1 chunked body from raw bytes read until close.
///
/// A truncated final chunk (the peer closed mid-chunk) yields the valid
/// prefix instead of an error, so callers always get decodable bytes.
fn decode_chunked(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < input.len() {
        let Some(rel) = input[i..].windows(2).position(|w| w == b"\r\n") else {
            break;
        };
        let size_line = &input[i..i + rel];
        let size_hex = size_line.split(|&b| b == b';').next().unwrap_or_default();
        let size = usize::from_str_radix(std::str::from_utf8(size_hex).unwrap_or("0"), 16)
            .unwrap_or(0);
        i += rel + 2;
        if size == 0 {
            break;
        }
        if i + size > input.len() {
            break;
        }
        out.extend_from_slice(&input[i..i + size]);
        i += size;
        if input.get(i) == Some(&b'\r') && input.get(i + 1) == Some(&b'\n') {
            i += 2;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};

    /// Fail a test without a panic macro (workspace forbids panic in tests).
    fn die(message: &str) -> ! {
        eprintln!("test setup: {message}");
        std::process::exit(1);
    }

    /// Read a request head: lines until the first line that is only the
    /// terminator. `read_line` appends, so the buffer is cleared per read.
    fn read_head(reader: &mut impl BufRead) {
        let mut line = String::new();
        loop {
            line.clear();
            let n = match reader.read_line(&mut line) {
                Ok(n) => n,
                Err(error) => die(&format!("read line: {error}")),
            };
            if n == 0 {
                die("client closed before the request head ended");
            }
            if line == "\r\n" {
                break;
            }
        }
    }

    /// Bind a loopback listener on a random port and return it with its URL.
    fn bind_loopback() -> (TcpListener, String) {
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(error) => die(&format!("bind: {error}")),
        };
        let addr = match listener.local_addr() {
            Ok(addr) => addr,
            Err(error) => die(&format!("addr: {error}")),
        };
        let url = format!("http://127.0.0.1:{}/probe", addr.port());
        (listener, url)
    }

    /// Spin up a one-shot loopback server that answers with a fixed body.
    fn one_shot_server(status_line: &str, headers: &str, body: &str) -> (String, thread::JoinHandle<()>) {
        let (listener, url) = bind_loopback();
        let status_line = status_line.to_string();
        let headers = headers.to_string();
        let body = body.to_string();
        let handle = thread::spawn(move || {
            let (mut sock, _) = match listener.accept() {
                Ok(pair) => pair,
                Err(error) => die(&format!("accept: {error}")),
            };
            let mut reader = BufReader::new(&sock);
            read_head(&mut reader);
            let payload = format!("{status_line}{headers}\r\n{body}");
            let _ = sock.write_all(payload.as_bytes());
        });
        (url, handle)
    }

    /// Run one request against a one-shot server and drop the server handle.
    fn probe(status_line: &str, headers: &str, body: &str) -> Response {
        let (url, handle) = one_shot_server(status_line, headers, body);
        let response = match request("GET", &url, &[], &[]) {
            Ok(response) => response,
            Err(error) => die(&format!("request: {error}")),
        };
        if handle.join().is_err() {
            die("server thread");
        }
        response
    }

    /// Spin up a loopback server that answers each connection with the next
    /// scripted `(status, extra_headers, body)` entry, repeating the last one
    /// when the script is exhausted. Returns the URL, a connection counter,
    /// and the server handle.
    fn scripted_server(
        script: Vec<(u16, &str, &str)>,
    ) -> (String, Arc<AtomicUsize>, thread::JoinHandle<()>) {
        let (listener, url) = bind_loopback();
        let seen = Arc::new(AtomicUsize::new(0));
        let script_owned: Vec<(u16, String, String)> = script
            .into_iter()
            .map(|(status, extra, body)| (status, extra.to_string(), body.to_string()))
            .collect();
        let counter = seen.clone();
        let handle = thread::spawn(move || serve_loop(&listener, &script_owned, &counter));
        (url, seen, handle)
    }

    /// Serve scripted responses until the listener is dropped.
    fn serve_loop(
        listener: &TcpListener,
        script: &[(u16, String, String)],
        counter: &Arc<AtomicUsize>,
    ) {
        for incoming in listener.incoming() {
            let Ok(mut sock) = incoming else {
                break;
            };
            let n = counter.fetch_add(1, Ordering::SeqCst) + 1;
            serve_one(&mut sock, script, n);
        }
    }

    /// Answer one connection with the next scripted entry.
    fn serve_one(sock: &mut TcpStream, script: &[(u16, String, String)], n: usize) {
        let Some((status, extra, body)) =
            script.get(n - 1).or_else(|| script.last())
        else {
            die("scripted_server: empty script");
        };
        let status_line = format!("HTTP/1.1 {status} X\r\n");
        let headers =
            format!("content-length: {}\r\nconnection: close\r\n{extra}", body.len());
        let payload = format!("{status_line}{headers}\r\n{body}");
        let mut reader = BufReader::new(&mut *sock);
        read_head(&mut reader);
        let _ = sock.write_all(payload.as_bytes());
    }

    #[test]
    fn decodes_content_length_body() {
        let resp = probe(
            "HTTP/1.1 200 OK\r\n",
            "content-length: 5\r\nconnection: close\r\n",
            "hello",
        );
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, b"hello");
    }

    #[test]
    fn decodes_chunked_body() {
        // Two chunks framing the same "hello" bytes.
        let framed = "3\r\nhel\r\n2\r\nlo\r\n0\r\n\r\n";
        let resp = probe(
            "HTTP/1.1 200 OK\r\n",
            "transfer-encoding: chunked\r\nconnection: close\r\n",
            framed,
        );
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, b"hello");
    }

    #[test]
    fn reports_status_of_error_responses() {
        let resp = probe(
            "HTTP/1.1 404 Not Found\r\n",
            "content-length: 2\r\nconnection: close\r\n",
            "{}",
        );
        assert_eq!(resp.status, 404);
        assert_eq!(resp.body, b"{}");
    }

    #[test]
    fn rejects_unsupported_schemes() {
        let err = request("GET", "ftp://example.com/", &[], &[])
            .err()
            .unwrap_or_default();
        assert!(err.contains("unsupported url"), "{err}");
    }

    #[test]
    fn retries_transient_status_then_succeeds() {
        let (url, seen, _handle) =
            scripted_server(vec![(503, "", "unavailable"), (200, "", "ok")]);
        let resp = request_with_attempts("GET", &url, &[], &[], 3)
            .unwrap_or_else(|e| die(&format!("request: {e}")));
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, b"ok");
        assert_eq!(seen.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn returns_last_retryable_response_after_budget() {
        let (url, seen, _handle) = scripted_server(vec![(503, "", "unavailable")]);
        let resp = request_with_attempts("GET", &url, &[], &[], 2)
            .unwrap_or_else(|e| die(&format!("request: {e}")));
        assert_eq!(resp.status, 503);
        assert_eq!(seen.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn does_not_retry_client_errors() {
        let (url, seen, _handle) = scripted_server(vec![(404, "", "nope")]);
        let resp = request_with_attempts("GET", &url, &[], &[], 3)
            .unwrap_or_else(|e| die(&format!("request: {e}")));
        assert_eq!(resp.status, 404);
        assert_eq!(seen.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn honors_retry_after_header() {
        let (url, seen, _handle) = scripted_server(vec![
            (429, "retry-after: 1\r\n", "slow down"),
            (200, "", "ok"),
        ]);
        let started = Instant::now();
        let resp = request_with_attempts("GET", &url, &[], &[], 2)
            .unwrap_or_else(|e| die(&format!("request: {e}")));
        let elapsed = started.elapsed();
        assert_eq!(resp.status, 200);
        assert_eq!(seen.load(Ordering::SeqCst), 2);
        assert!(elapsed >= Duration::from_millis(900), "slept only {elapsed:?}");
    }
}
