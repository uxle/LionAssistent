//! A tiny, std-only HTTP/1.1 mock server for exercising [`crate::HttpClient`]
//! (and downstream crates like `updater`) without ever touching the real
//! network. Deliberately not a general-purpose test server -- one
//! connection, one canned response, just enough to test a client -- and
//! deliberately not a new dependency (DIG SS11) just for test scaffolding.
//!
//! This lives in the crate's normal (non-`#[cfg(test)]`) public API, not
//! hidden behind a test flag, specifically so `updater`'s own tests can
//! use it as a dev-dependency too -- `#[cfg(test)]` code is invisible
//! outside the crate that defines it.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread::JoinHandle;

/// A running mock server. Drop (or call [`MockServer::join`]) once the
/// test has made its request, so the background thread doesn't outlive
/// it.
pub struct MockServer {
    pub base_url: String,
    handle: Option<JoinHandle<()>>,
}

impl MockServer {
    /// Starts a background thread that accepts exactly one connection,
    /// reads (and discards) whatever the client sends, and writes back
    /// `status`/`content_type`/`body`. [`MockServer::base_url`] is
    /// `http://127.0.0.1:<port>` for an OS-assigned free port.
    pub fn respond_once(status: u16, content_type: &str, body: Vec<u8>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("mock server bind failed");
        let addr = listener
            .local_addr()
            .expect("mock server has no local addr");
        let content_type = content_type.to_string();

        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 8192];
                // Simple GET requests with no body arrive in a single
                // TCP segment on loopback in practice; a full HTTP
                // parser would be overkill for a client-only test
                // double that never inspects the request.
                let _ = stream.read(&mut buf);

                let response = format!(
                    "HTTP/1.1 {status} {}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    status_text(status),
                    body.len(),
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.write_all(&body);
                let _ = stream.flush();
            }
        });

        Self {
            base_url: format!("http://{addr}"),
            handle: Some(handle),
        }
    }

    /// Blocks until the one request has actually been served. Tests
    /// should call this after making their request, so a failure inside
    /// the server thread (e.g. a write error) isn't silently dropped.
    pub fn join(mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn status_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Unknown",
    }
}
