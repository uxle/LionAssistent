//! The HTTP/SSE server itself (Development_plan Phase 28 "Remote
//! Control"). Every route except the static PWA shell and
//! `/pair/complete` requires a valid paired-device bearer token
//! ([`crate::pairing::DeviceStore::authenticate`]); `/tools/execute`
//! additionally routes through `ToolRouter::execute` -- the exact same
//! validate/permission-check/timeout pipeline any local caller already
//! goes through (TCLIS SS1) -- so a remote, paired device gets no more
//! trust than a local one, only a different transport to reach the
//! same gate.
//!
//! Built on `tiny_http` (sync, blocking -- verified against the pinned
//! Rust 1.75 toolchain before use, the same empirical-first check
//! every dependency in this workspace gets). `ToolRouter::execute` is
//! `async`; this crate bridges into it from a synchronous request
//! thread via an explicitly-passed `tokio::runtime::Handle`, the exact
//! same pattern `mcp::server::serve` already established for the
//! identical constraint (Phase 27) -- not a new idiom invented here.

use crate::notify::NotificationRelay;
use crate::pairing::{DeviceStore, PairingManager, RateLimiter};
use common::AppResult;
use serde_json::json;
use std::io::Write;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};
use tool_framework::{ToolArgs, ToolRouter};

/// How many `/pair/complete` attempts one remote address may make per
/// [`PAIR_RATE_LIMIT_WINDOW`] -- see `pairing`'s module docs for how
/// this and [`crate::pairing::MAX_PAIRING_ATTEMPTS`] work together.
/// `pub` for the same reason `pairing::PAIRING_CODE_TTL` and
/// `pairing::MAX_PAIRING_ATTEMPTS` are: a real, stable security
/// parameter worth being visible and linkable, not an internal detail.
pub const PAIR_RATE_LIMIT_MAX: u32 = 10;
pub const PAIR_RATE_LIMIT_WINDOW: Duration = Duration::from_secs(5 * 60);

/// Everything one running server needs, grouped so `serve` doesn't
/// take an unwieldy parameter list. Every field is already
/// internally synchronized (`Mutex`/`Arc`), so `&SharedState` is
/// enough for every route handler despite requests being handled on
/// multiple threads.
pub struct SharedState {
    pub devices: Arc<DeviceStore>,
    pub pairing: Arc<PairingManager>,
    pub router: ToolRouter,
    pub notifications: NotificationRelay,
    pub pair_rate_limiter: RateLimiter,
    pub runtime_handle: tokio::runtime::Handle,
}

impl SharedState {
    /// Builds `SharedState` with the pairing rate limiter configured
    /// to this crate's own defaults ([`PAIR_RATE_LIMIT_MAX`] per
    /// [`PAIR_RATE_LIMIT_WINDOW`]) -- the ordinary way to construct
    /// this outside a test that specifically wants a different limit.
    ///
    /// Takes `devices` and `pairing` as `Arc`s specifically so a
    /// caller (a future local settings UI, or a test) can keep its own
    /// clone of each and drive pairing through the *exact* instance
    /// the server's request handlers see -- `begin_pairing` only means
    /// anything if it's called on the same `PairingManager`
    /// `/pair/complete` checks against.
    pub fn new(
        devices: Arc<DeviceStore>,
        pairing: Arc<PairingManager>,
        router: ToolRouter,
        notifications: NotificationRelay,
        runtime_handle: tokio::runtime::Handle,
    ) -> Self {
        Self {
            devices,
            pairing,
            router,
            notifications,
            pair_rate_limiter: RateLimiter::new(PAIR_RATE_LIMIT_WINDOW, PAIR_RATE_LIMIT_MAX),
            runtime_handle,
        }
    }
}

/// True for a private (RFC 1918), loopback, or link-local address --
/// i.e. one that cannot have reached this process from the public
/// internet without a NAT/router explicitly configured to allow it.
/// Used only to gate `/pair/complete` (see `pairing`'s module docs);
/// every other route is already behind bearer-token auth regardless of
/// where the request came from.
fn is_local_network_address(addr: IpAddr) -> bool {
    match addr {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || (v6.segments()[0] & 0xfe00) == 0xfc00 // fc00::/7 unique local
                || (v6.segments()[0] & 0xffc0) == 0xfe80 // fe80::/10 link-local
        }
    }
}

fn json_response(status: u16, body: serde_json::Value) -> Response<std::io::Cursor<Vec<u8>>> {
    let bytes = serde_json::to_vec(&body).unwrap_or_else(|_| b"{}".to_vec());
    Response::from_data(bytes)
        .with_status_code(StatusCode(status))
        .with_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap())
}

fn error_response(status: u16, message: impl Into<String>) -> Response<std::io::Cursor<Vec<u8>>> {
    json_response(status, json!({"error": message.into()}))
}

fn read_body_json(request: &mut Request) -> Result<serde_json::Value, String> {
    let mut buf = String::new();
    request
        .as_reader()
        .read_to_string(&mut buf)
        .map_err(|e| format!("could not read request body: {e}"))?;
    if buf.is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(&buf).map_err(|e| format!("invalid JSON body: {e}"))
}

/// Extracts the bearer token from the `Authorization` header. Every
/// route except `/events` (see [`sse_token`]) uses only this -- a
/// token that can only ever arrive via a header never ends up in a
/// URL, and therefore never in a server access log or browser history
/// either.
fn bearer_token(request: &Request) -> Option<String> {
    request
        .headers()
        .iter()
        .find(|h| {
            h.field
                .as_str()
                .as_str()
                .eq_ignore_ascii_case("Authorization")
        })
        .map(|h| h.value.as_str())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::to_string)
}

/// Extracts the bearer token for the `/events` route specifically:
/// the `Authorization` header if present, else a `?token=` query
/// parameter.
///
/// The query-parameter fallback exists for exactly one reason and is
/// deliberately not [`bearer_token`]'s general behavior: the browser
/// `EventSource` API (what `assets/app.js` uses for SSE) has no
/// mechanism to set custom request headers at all, so a header-only
/// scheme would make `/events` -- and only `/events` -- unreachable
/// from the one client this crate actually ships. It carries the same
/// value already stored in `localStorage` on the client, not a
/// separate, weaker credential, but every other route keeps the
/// stricter header-only rule since they have no such constraint.
fn sse_token(request: &Request) -> Option<String> {
    bearer_token(request).or_else(|| token_from_query(request.url()))
}

fn token_from_query(url: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key == "token").then(|| percent_decode(value))
    })
}

/// Minimal percent-decoding for the one query parameter this crate
/// ever reads (`token`) -- not a general-purpose URL decoder. A device
/// token is 64 hex characters (see `pairing::generate_device_token`),
/// which contains nothing that needs escaping in practice, but a
/// client is still free to percent-encode it, so this handles `%XX`
/// correctly rather than assuming it never occurs.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Serves one already-accepted connection's requests to completion.
/// `tiny_http`'s own `Server::recv` gives one `Request` per HTTP
/// request (it handles keep-alive internally), so this is a loop over
/// requests, not connections; called from a worker thread spawned by
/// [`serve`].
fn handle_request(state: &Arc<SharedState>, mut request: Request) {
    let method = request.method().clone();
    let full_url = request.url().to_string();
    let path = full_url.split('?').next().unwrap_or(&full_url).to_string();
    let remote_addr = request.remote_addr().map(|a| a.ip());

    let response_result = match (&method, path.as_str()) {
        (Method::Post, "/pair/complete") => handle_pair_complete(state, &mut request, remote_addr),
        (Method::Get, "/status") => with_auth(state, &request, |device| {
            json_response(
                200,
                json!({"paired_as": device.label, "server": "ai-assistant-remote-control"}),
            )
        }),
        (Method::Get, "/tools") => with_auth(state, &request, |_device| {
            let names: Vec<&str> = state.router.registry().list();
            let tools: Vec<_> = names
                .iter()
                .filter_map(|n| state.router.registry().get(n))
                .map(|t| {
                    json!({
                        "name": t.name(),
                        "description": t.description(),
                        "permission_level": format!("{:?}", t.permission_level()),
                    })
                })
                .collect();
            json_response(200, json!({"tools": tools}))
        }),
        (Method::Post, "/tools/execute") => handle_tools_execute(state, &mut request),
        (Method::Get, "/events") => {
            return handle_events_sse(state, request);
        }
        (Method::Get, "/") => serve_asset("index.html", "text/html; charset=utf-8"),
        (Method::Get, "/app.js") => serve_asset("app.js", "application/javascript"),
        (Method::Get, "/style.css") => serve_asset("style.css", "text/css"),
        (Method::Get, "/manifest.json") => {
            serve_asset("manifest.json", "application/manifest+json")
        }
        _ => error_response(404, format!("no route for {method:?} {path}")),
    };

    if let Err(e) = request.respond(response_result) {
        tracing::warn!(error = %e, "failed to write HTTP response");
    }
}

fn with_auth(
    state: &Arc<SharedState>,
    request: &Request,
    handler: impl FnOnce(crate::pairing::Device) -> Response<std::io::Cursor<Vec<u8>>>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let Some(token) = bearer_token(request) else {
        return error_response(401, "missing Authorization: Bearer <token> header");
    };
    match state.devices.authenticate(&token) {
        Ok(Some(device)) => handler(device),
        Ok(None) => error_response(401, "invalid or revoked device token"),
        Err(e) => error_response(500, format!("internal error checking device token: {e}")),
    }
}

fn handle_pair_complete(
    state: &Arc<SharedState>,
    request: &mut Request,
    remote_addr: Option<IpAddr>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let Some(addr) = remote_addr else {
        return error_response(403, "could not determine remote address");
    };
    if !is_local_network_address(addr) {
        return error_response(
            403,
            "pairing is only accepted from the local network (see pairing.rs docs)",
        );
    }
    if !state.pair_rate_limiter.check(&addr.to_string()) {
        return error_response(
            429,
            "too many pairing attempts from this address; slow down",
        );
    }

    let body = match read_body_json(request) {
        Ok(b) => b,
        Err(e) => return error_response(400, e),
    };
    let Some(code) = body.get("code").and_then(|v| v.as_str()) else {
        return error_response(400, "missing \"code\" field");
    };
    let device_label = body
        .get("device_label")
        .and_then(|v| v.as_str())
        .unwrap_or("Unnamed device");

    match state.pairing.try_complete(code) {
        Ok(_label) => match state.devices.register(device_label) {
            Ok((device_id, token)) => {
                json_response(200, json!({"device_id": device_id, "token": token}))
            }
            Err(e) => error_response(
                500,
                format!("pairing succeeded but registration failed: {e}"),
            ),
        },
        Err(e) => error_response(401, e.to_string()),
    }
}

fn handle_tools_execute(
    state: &Arc<SharedState>,
    request: &mut Request,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let Some(token) = bearer_token(request) else {
        return error_response(401, "missing Authorization: Bearer <token> header");
    };
    let device = match state.devices.authenticate(&token) {
        Ok(Some(d)) => d,
        Ok(None) => return error_response(401, "invalid or revoked device token"),
        Err(e) => return error_response(500, format!("internal error checking device token: {e}")),
    };

    let body = match read_body_json(request) {
        Ok(b) => b,
        Err(e) => return error_response(400, e),
    };
    let Some(tool_name) = body.get("tool").and_then(|v| v.as_str()) else {
        return error_response(400, "missing \"tool\" field");
    };
    let args: ToolArgs = match body.get("args") {
        Some(serde_json::Value::Object(map)) => map.clone().into_iter().collect(),
        None => ToolArgs::new(),
        Some(_) => return error_response(400, "\"args\" must be a JSON object"),
    };

    tracing::info!(device = %device.label, tool = tool_name, "remote tool execution requested");
    let result = state
        .runtime_handle
        .block_on(state.router.execute(tool_name, args));

    json_response(
        200,
        json!({
            "success": result.success,
            "status": format!("{:?}", result.status),
            "message": result.message,
            "data": result.data,
            "execution_time_ms": result.execution_time.as_secs_f64() * 1000.0,
        }),
    )
}

/// Server-Sent Events stream of relayed `event-bus` events (Phase 28
/// "Notifications"). Uses `Request::into_writer` -- the escape hatch
/// out of `tiny_http`'s ordinary one-`Response`-per-`Request` model --
/// to hold the connection open and keep writing as events arrive,
/// rather than one bounded response. Consumes `request` because
/// `into_writer` does; this is why `handle_request` returns early for
/// this route instead of producing an ordinary `Response`.
///
/// The response is framed with `Transfer-Encoding: chunked`, written
/// by hand one chunk per SSE message. This is not optional decoration:
/// an HTTP/1.1 response with neither `Content-Length` nor
/// `Transfer-Encoding` has genuinely ambiguous body framing under the
/// spec once keep-alive is in play (as opposed to plain HTTP/1.0,
/// where "read until the connection closes" is the well-defined
/// fallback) -- a strict client has no correct way to know a
/// zero-`Content-Length`-looking response actually has more data
/// coming. This was found the empirical way, not reasoned out in
/// advance: an early version of this handler wrote raw, unframed
/// bytes and the real client/server round-trip test hung waiting on
/// `reader.read_line`, exactly the failure mode this framing exists
/// to rule out.
fn handle_events_sse(state: &Arc<SharedState>, request: Request) {
    let Some(token) = sse_token(&request) else {
        let _ = request.respond(error_response(401, "missing bearer token"));
        return;
    };
    let device = match state.devices.authenticate(&token) {
        Ok(Some(d)) => d,
        Ok(None) => {
            let _ = request.respond(error_response(401, "invalid or revoked device token"));
            return;
        }
        Err(e) => {
            let _ = request.respond(error_response(500, e.to_string()));
            return;
        }
    };

    let receiver = state.notifications.subscribe();
    let mut writer = request.into_writer();
    let preamble = b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\nTransfer-Encoding: chunked\r\n\r\n";
    // The explicit flush here is load-bearing, not decoration: without
    // it, these bytes can sit in an internal buffer indefinitely,
    // since nothing else forces a flush until the first SSE event
    // arrives -- and the client is blocked waiting for these headers
    // *before* any event exists to trigger that write. Found by an
    // actual hang in the real client/server round-trip test, not
    // reasoned out in advance -- see that test file's own note.
    if writer.write_all(preamble).is_err() || writer.flush().is_err() {
        return;
    }
    tracing::info!(device = %device.label, "SSE notification stream opened");

    // Blocking recv on a std::sync::mpsc-backed relay (see notify.rs):
    // this thread exists specifically to sit here for the life of the
    // connection, exactly like every other tiny_http worker thread
    // sits blocked in server.recv() between requests.
    while let Ok(event_json) = receiver.recv() {
        let sse_line = format!("data: {event_json}\n\n");
        if write_chunk(&mut writer, sse_line.as_bytes()).is_err() {
            break; // client disconnected
        }
    }
    let _ = write_final_chunk(&mut writer);
    tracing::info!(device = %device.label, "SSE notification stream closed");
}

/// Writes one `Transfer-Encoding: chunked` chunk: the size in hex,
/// CRLF, the payload, CRLF -- RFC 9112 SS7.1.
fn write_chunk(writer: &mut dyn Write, payload: &[u8]) -> std::io::Result<()> {
    write!(writer, "{:x}\r\n", payload.len())?;
    writer.write_all(payload)?;
    writer.write_all(b"\r\n")?;
    writer.flush()
}

/// The zero-length chunk that terminates a chunked response body.
fn write_final_chunk(writer: &mut dyn Write) -> std::io::Result<()> {
    writer.write_all(b"0\r\n\r\n")?;
    writer.flush()
}

fn serve_asset(name: &str, content_type: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let bytes = crate::assets::get(name);
    Response::from_data(bytes.to_vec())
        .with_status_code(StatusCode(200))
        .with_header(Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes()).unwrap())
}

/// Runs the server until the process exits. Binds `bind_address:port`
/// (see `config::RemoteControlConfig`'s doc comment for why the
/// default is loopback-only) and spawns `worker_threads` blocking
/// worker threads pulling from the shared `tiny_http::Server`, the
/// same fan-out pattern `tiny_http`'s own docs recommend.
pub fn serve(
    bind_address: &str,
    port: u16,
    state: SharedState,
    worker_threads: usize,
) -> AppResult<()> {
    let server =
        Server::http(format!("{bind_address}:{port}")).map_err(|e| common::AppError::Module {
            module: "remote-control",
            message: format!("could not bind {bind_address}:{port}: {e}"),
        })?;
    let server = Arc::new(server);
    let state = Arc::new(state);

    tracing::info!(bind_address, port, "remote-control server listening");

    let mut handles = Vec::with_capacity(worker_threads);
    for _ in 0..worker_threads.max(1) {
        let server = Arc::clone(&server);
        let state = Arc::clone(&state);
        handles.push(std::thread::spawn(move || {
            while let Ok(request) = server.recv() {
                handle_request(&state, request);
            }
        }));
    }
    for h in handles {
        let _ = h.join();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_and_private_ranges_are_local() {
        assert!(is_local_network_address("127.0.0.1".parse().unwrap()));
        assert!(is_local_network_address("192.168.1.50".parse().unwrap()));
        assert!(is_local_network_address("10.0.0.5".parse().unwrap()));
        assert!(is_local_network_address("172.16.0.1".parse().unwrap()));
        assert!(is_local_network_address("169.254.1.1".parse().unwrap()));
        assert!(is_local_network_address("::1".parse().unwrap()));
        assert!(is_local_network_address(
            "fd12:3456:789a::1".parse().unwrap()
        ));
    }

    #[test]
    fn public_addresses_are_not_local() {
        assert!(!is_local_network_address("8.8.8.8".parse().unwrap()));
        assert!(!is_local_network_address("1.1.1.1".parse().unwrap()));
        assert!(!is_local_network_address(
            "2606:4700:4700::1111".parse().unwrap() // public IPv6 (Cloudflare)
        ));
    }

    #[test]
    fn public_ip_boundary_just_outside_private_ranges_is_rejected() {
        // 172.32.0.0 is one /12 block past the 172.16.0.0/12 private
        // range -- a real boundary check, not just "starts with 172.".
        assert!(!is_local_network_address("172.32.0.1".parse().unwrap()));
        // 192.169.0.0 is one class past 192.168.0.0/16.
        assert!(!is_local_network_address("192.169.0.1".parse().unwrap()));
    }
}
