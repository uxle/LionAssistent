//! `HttpClient` -- Phase 14's "HTTP Client" module (Development_plan
//! Phase 14). Synchronous by design, matching every other stateful
//! service in this workspace (`MemoryStore`, `WorkflowStore`,
//! `PermissionManager` are all plain sync structs too): async only
//! shows up at the `Tool::execute`/`ToolRouter` boundary, via
//! `spawn_blocking`, not inside individual crates. A tool wrapping this
//! client (see the `updater` crate) fits that existing pattern directly,
//! rather than needing a runtime `Handle` threaded through just for one
//! new capability.

use common::{AppError, AppResult};
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_USER_AGENT: &str =
    "ai-assistant (Local Linux AI Assistant; Development_plan Phase 14)";
/// Refuses to buffer or write more than this many bytes for a single
/// response -- a deliberately generous but finite cap (TCLIS SS7
/// "resource limits"), so a misbehaving or malicious server can't turn
/// a `get`/`download` call into an unbounded memory allocation or an
/// unbounded write to disk.
const DEFAULT_MAX_RESPONSE_BYTES: u64 = 200 * 1024 * 1024; // 200 MiB

/// A plain HTTP response: whatever the server actually said, including
/// error statuses. See [`HttpClient::get`]'s doc comment for why 4xx/5xx
/// responses are `Ok`, not `Err`, here.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

impl HttpResponse {
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// What [`HttpClient::download`] reports after streaming a response body
/// to disk.
#[derive(Debug, Clone)]
pub struct DownloadOutcome {
    pub status: u16,
    pub bytes_written: u64,
    pub content_type: Option<String>,
}

pub struct HttpClient {
    timeout: Duration,
    user_agent: String,
    max_response_bytes: u64,
    agent: ureq::Agent,
}

impl HttpClient {
    /// Builds a client using the OS's own certificate trust store (see
    /// `system_tls_config`), a 30-second timeout, and an identifying
    /// User-Agent -- some real-world APIs (crates.io's among them)
    /// reject requests that don't send one at all.
    pub fn new() -> AppResult<Self> {
        Self::build(
            DEFAULT_TIMEOUT,
            DEFAULT_USER_AGENT.to_string(),
            DEFAULT_MAX_RESPONSE_BYTES,
        )
    }

    pub fn with_timeout(self, timeout: Duration) -> AppResult<Self> {
        Self::build(timeout, self.user_agent, self.max_response_bytes)
    }

    pub fn with_user_agent(self, user_agent: impl Into<String>) -> AppResult<Self> {
        Self::build(self.timeout, user_agent.into(), self.max_response_bytes)
    }

    pub fn with_max_response_bytes(self, max_response_bytes: u64) -> AppResult<Self> {
        Self::build(self.timeout, self.user_agent, max_response_bytes)
    }

    fn build(timeout: Duration, user_agent: String, max_response_bytes: u64) -> AppResult<Self> {
        let agent = ureq::AgentBuilder::new()
            .timeout(timeout)
            .tls_config(std::sync::Arc::new(system_tls_config()?))
            .build();
        Ok(Self {
            timeout,
            user_agent,
            max_response_bytes,
            agent,
        })
    }

    /// Performs a `GET`. Any response the server actually sends back --
    /// including a 404 or a 500 -- comes back as `Ok`; only a genuine
    /// transport failure (DNS, connection refused, TLS, timeout) is
    /// `Err`. A generic HTTP client shouldn't bake in the opinion that a
    /// non-2xx status is exceptional -- that's the caller's policy to
    /// decide (`updater`, for instance, treats a 404 manifest as "no
    /// update information available," not a hard error).
    pub fn get(&self, url: &str) -> AppResult<HttpResponse> {
        match self
            .agent
            .get(url)
            .set("User-Agent", &self.user_agent)
            .call()
        {
            Ok(response) => self.read_body(response),
            Err(ureq::Error::Status(_, response)) => self.read_body(response),
            Err(ureq::Error::Transport(t)) => Err(transport_error(t)),
        }
    }

    /// `GET` and parse the body as JSON. Only succeeds for a 2xx status
    /// with a body that actually parses -- unlike [`HttpClient::get`],
    /// there's no reasonable typed value to hand back for an error
    /// status, so this collapses that case into `Err` for convenience.
    pub fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> AppResult<T> {
        let response = self.get(url)?;
        if !response.is_success() {
            return Err(AppError::Module {
                module: "network",
                message: format!("GET {url} returned status {}", response.status),
            });
        }
        serde_json::from_str(&response.body)
            .map_err(|e| AppError::Serialization(format!("parsing response from {url}: {e}")))
    }

    /// `POST` a JSON body and parse a JSON response.
    pub fn post_json<B: serde::Serialize, T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        body: &B,
    ) -> AppResult<T> {
        let json_body =
            serde_json::to_value(body).map_err(|e| AppError::Serialization(e.to_string()))?;
        let result = self
            .agent
            .post(url)
            .set("User-Agent", &self.user_agent)
            .send_json(json_body);
        let response = match result {
            Ok(response) => self.read_body(response)?,
            Err(ureq::Error::Status(_, response)) => self.read_body(response)?,
            Err(ureq::Error::Transport(t)) => return Err(transport_error(t)),
        };
        if !response.is_success() {
            return Err(AppError::Module {
                module: "network",
                message: format!("POST {url} returned status {}", response.status),
            });
        }
        serde_json::from_str(&response.body)
            .map_err(|e| AppError::Serialization(format!("parsing response from {url}: {e}")))
    }

    /// Streams a `GET` response body straight to `dest`, never buffering
    /// more than [`HttpClient::with_max_response_bytes`] worth in
    /// memory. `dest`'s parent directory must already exist -- this
    /// mirrors `linux_services::files::write`'s own "the caller
    /// specifies exactly where, this function doesn't guess" philosophy.
    pub fn download(&self, url: &str, dest: &Path) -> AppResult<DownloadOutcome> {
        let response = match self
            .agent
            .get(url)
            .set("User-Agent", &self.user_agent)
            .call()
        {
            Ok(response) => response,
            Err(ureq::Error::Status(_, response)) => response,
            Err(ureq::Error::Transport(t)) => return Err(transport_error(t)),
        };
        let status = response.status();
        let content_type = response.header("Content-Type").map(str::to_string);

        let mut file = std::fs::File::create(dest)?;
        let mut reader = response.into_reader().take(self.max_response_bytes + 1);
        let mut buf = [0u8; 64 * 1024];
        let mut bytes_written: u64 = 0;

        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            bytes_written += n as u64;
            if bytes_written > self.max_response_bytes {
                drop(file);
                let _ = std::fs::remove_file(dest);
                return Err(AppError::Module {
                    module: "network",
                    message: format!(
                        "response from {url} exceeded the {}-byte limit; download aborted and partial file removed",
                        self.max_response_bytes
                    ),
                });
            }
            file.write_all(&buf[..n])?;
        }
        file.flush()?;

        Ok(DownloadOutcome {
            status,
            bytes_written,
            content_type,
        })
    }

    fn read_body(&self, response: ureq::Response) -> AppResult<HttpResponse> {
        let status = response.status();
        let mut body = String::new();
        response
            .into_reader()
            .take(self.max_response_bytes + 1)
            .read_to_string(&mut body)
            .map_err(AppError::Io)?;
        if body.len() as u64 > self.max_response_bytes {
            return Err(AppError::Module {
                module: "network",
                message: format!(
                    "response exceeded the {}-byte limit",
                    self.max_response_bytes
                ),
            });
        }
        Ok(HttpResponse { status, body })
    }
}

/// A `rustls::ClientConfig` that trusts the OS's own certificate store
/// (via `rustls-native-certs`) instead of the fixed list `rustls`/`ureq`
/// bundle in by default.
///
/// This isn't just a nice-to-have for a Linux *desktop* app (where the
/// OS trust store is the one thing that actually gets updated by the
/// system, independent of this binary's release cycle) -- it's what
/// made real HTTPS requests work *at all* while building this phase.
/// `ureq`'s default (bundled `webpki-roots`) failed real requests in
/// this sandbox with `InvalidCertificate(UnknownIssuer)`, while `curl`
/// against the same host succeeded immediately -- `curl` reads the
/// system store (`SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt` is
/// set in this environment for exactly that reason), so pointing
/// `rustls` at the same store was the fix, not a coincidence.
fn system_tls_config() -> AppResult<ureq::rustls::ClientConfig> {
    let mut roots = ureq::rustls::RootCertStore::empty();
    let loaded = rustls_native_certs::load_native_certs().map_err(|e| AppError::Module {
        module: "network",
        message: format!("loading system certificate store: {e}"),
    })?;
    for cert in loaded {
        roots.add(cert).map_err(|e| AppError::Module {
            module: "network",
            message: format!("invalid system certificate: {e}"),
        })?;
    }
    Ok(ureq::rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth())
}

fn transport_error(t: ureq::Transport) -> AppError {
    AppError::Module {
        module: "network",
        message: t.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::MockServer;
    use serde::{Deserialize, Serialize};

    #[test]
    fn get_returns_a_successful_response() {
        let server = MockServer::respond_once(200, "text/plain", b"hello".to_vec());
        let client = HttpClient::new().unwrap();
        let response = client.get(&server.base_url).unwrap();
        server.join();

        assert_eq!(response.status, 200);
        assert_eq!(response.body, "hello");
        assert!(response.is_success());
    }

    #[test]
    fn get_returns_ok_for_error_statuses_not_err() {
        let server = MockServer::respond_once(404, "text/plain", b"not found".to_vec());
        let client = HttpClient::new().unwrap();
        let response = client.get(&server.base_url).unwrap();
        server.join();

        assert_eq!(response.status, 404);
        assert!(!response.is_success());
    }

    #[test]
    fn get_is_err_for_a_real_transport_failure() {
        // Nothing is listening on this port -- connection refused, a
        // genuine transport-level failure, not a status code.
        let client = HttpClient::new().unwrap();
        let result = client.get("http://127.0.0.1:1");
        assert!(result.is_err());
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Payload {
        name: String,
        count: u32,
    }

    #[test]
    fn get_json_parses_a_successful_body() {
        let payload = Payload {
            name: "widgets".into(),
            count: 3,
        };
        let server = MockServer::respond_once(
            200,
            "application/json",
            serde_json::to_vec(&payload).unwrap(),
        );
        let client = HttpClient::new().unwrap();
        let parsed: Payload = client.get_json(&server.base_url).unwrap();
        server.join();

        assert_eq!(parsed, payload);
    }

    #[test]
    fn get_json_errors_on_a_non_success_status() {
        let server = MockServer::respond_once(500, "text/plain", b"oops".to_vec());
        let client = HttpClient::new().unwrap();
        let result: AppResult<Payload> = client.get_json(&server.base_url);
        server.join();

        assert!(result.is_err());
    }

    #[test]
    fn download_streams_the_body_to_a_file() {
        let dir = std::env::temp_dir().join("network-download-test-basic");
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("out.bin");

        let content = b"the quick brown fox jumps over the lazy dog".to_vec();
        let server = MockServer::respond_once(200, "application/octet-stream", content.clone());
        let client = HttpClient::new().unwrap();
        let outcome = client.download(&server.base_url, &dest).unwrap();
        server.join();

        assert_eq!(outcome.status, 200);
        assert_eq!(outcome.bytes_written, content.len() as u64);
        assert_eq!(std::fs::read(&dest).unwrap(), content);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn download_refuses_a_response_over_the_configured_limit() {
        let dir = std::env::temp_dir().join("network-download-test-oversized");
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("out.bin");

        let content = vec![b'x'; 1000];
        let server = MockServer::respond_once(200, "application/octet-stream", content);
        let client = HttpClient::new()
            .unwrap()
            .with_max_response_bytes(100)
            .unwrap();
        let result = client.download(&server.base_url, &dest);
        server.join();

        assert!(result.is_err());
        assert!(!dest.exists(), "partial file should have been cleaned up");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn builder_methods_actually_change_behavior() {
        let client = HttpClient::new()
            .unwrap()
            .with_user_agent("custom-agent/1.0")
            .unwrap()
            .with_timeout(Duration::from_millis(500))
            .unwrap();
        assert_eq!(client.user_agent, "custom-agent/1.0");
        assert_eq!(client.timeout, Duration::from_millis(500));
    }
}
