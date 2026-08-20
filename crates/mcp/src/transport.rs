//! Line-delimited JSON-RPC framing over any `BufRead`/`Write` pair --
//! the stdio transport (MCP "Transports": one message per line, no
//! embedded newlines). Confirmed against a real server during manual
//! probing: every message `@modelcontextprotocol/server-filesystem`
//! wrote was exactly one `\n`-terminated line.
//!
//! Generic over the reader/writer so the same type serves three
//! different real cases: a spawned child's stdio pipes
//! (`client::McpClient`), this process's own stdin/stdout
//! (`server::serve_stdio`), and an in-memory `Cursor<Vec<u8>>` pair in
//! tests -- framing itself is fully testable without a subprocess.

use common::{AppError, AppResult};
use serde_json::Value;
use std::io::{BufRead, Read, Write};

/// Refuses to buffer more than this many bytes looking for a line
/// terminator. Local IPC with a well-behaved peer never approaches
/// this; it exists so a broken or hostile peer streaming data with no
/// `\n` cannot grow this process's memory without bound (TCLIS SS7
/// "resource limits", SS12 performance requirements).
pub const MAX_LINE_BYTES: usize = 16 * 1024 * 1024;

pub struct LineTransport<R, W> {
    reader: R,
    writer: W,
}

impl<R: BufRead, W: Write> LineTransport<R, W> {
    pub fn new(reader: R, writer: W) -> Self {
        Self { reader, writer }
    }

    /// Consumes the transport and returns the writer half. Chiefly
    /// useful in tests, to inspect everything written to an in-memory
    /// buffer after driving a full exchange.
    pub fn into_writer(self) -> W {
        self.writer
    }

    /// Serializes `value` compactly and writes it as one
    /// `\n`-terminated line, then flushes. serde_json's compact output
    /// escapes any newline inside a string value as the two characters
    /// `\` `n`, so the one-message-per-line invariant holds by
    /// construction, not merely by convention.
    pub fn send(&mut self, value: &Value) -> AppResult<()> {
        let mut line =
            serde_json::to_string(value).map_err(|e| AppError::Serialization(e.to_string()))?;
        debug_assert!(
            !line.contains('\n'),
            "serde_json compact output must never contain a literal newline"
        );
        line.push('\n');
        self.writer.write_all(line.as_bytes())?;
        self.writer.flush()?;
        Ok(())
    }

    /// Reads one line and parses it as JSON.
    ///
    /// `Ok(None)` means clean EOF (the peer closed its output stream
    /// with no partial data pending) -- a normal shutdown signal, not
    /// an error. A bare blank line is skipped rather than treated as a
    /// parse failure, since it is harmless framing noise, not
    /// malformed JSON-RPC.
    pub fn recv(&mut self) -> AppResult<Option<Value>> {
        loop {
            let mut buf = Vec::new();
            let n = (&mut self.reader)
                .take(MAX_LINE_BYTES as u64 + 1)
                .read_until(b'\n', &mut buf)?;
            if n == 0 {
                return Ok(None);
            }
            if buf.len() > MAX_LINE_BYTES {
                return Err(AppError::Module {
                    module: "mcp",
                    message: format!(
                        "JSON-RPC line exceeded {MAX_LINE_BYTES} bytes without a newline"
                    ),
                });
            }
            while matches!(buf.last(), Some(b'\n') | Some(b'\r')) {
                buf.pop();
            }
            if buf.is_empty() {
                continue;
            }
            let text = String::from_utf8(buf).map_err(|e| AppError::Module {
                module: "mcp",
                message: format!("JSON-RPC line was not valid UTF-8: {e}"),
            })?;
            let value: Value = serde_json::from_str(&text).map_err(|e| AppError::Module {
                module: "mcp",
                message: format!("invalid JSON-RPC line: {e}"),
            })?;
            return Ok(Some(value));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Cursor;

    fn transport(input: &str) -> LineTransport<Cursor<Vec<u8>>, Vec<u8>> {
        LineTransport::new(Cursor::new(input.as_bytes().to_vec()), Vec::new())
    }

    #[test]
    fn round_trips_a_message_through_an_in_memory_pipe() {
        let mut t = LineTransport::new(Cursor::new(Vec::new()), Vec::new());
        t.send(&json!({"jsonrpc": "2.0", "id": 1, "method": "ping"}))
            .unwrap();
        // JSON objects are unordered by definition, and serde_json's
        // `Value::Object` (no `preserve_order` feature enabled in this
        // workspace) sorts keys alphabetically when serializing --
        // comparing parsed values is the correct check; comparing raw
        // bytes would be asserting an implementation detail this crate
        // makes no promise about.
        let written = t.writer.clone();
        let written_text = String::from_utf8(written.clone()).unwrap();
        assert_eq!(written_text.matches('\n').count(), 1);
        let reparsed: Value = serde_json::from_str(written_text.trim_end()).unwrap();
        assert_eq!(
            reparsed,
            json!({"jsonrpc": "2.0", "id": 1, "method": "ping"})
        );

        let mut reader_side = LineTransport::new(Cursor::new(written), Vec::new());
        let received = reader_side.recv().unwrap().unwrap();
        assert_eq!(received["method"], json!("ping"));
    }

    #[test]
    fn recv_returns_none_on_clean_eof() {
        let mut t = transport("");
        assert!(t.recv().unwrap().is_none());
    }

    #[test]
    fn recv_skips_blank_lines_between_messages() {
        let mut t = transport("\n\n{\"jsonrpc\":\"2.0\",\"method\":\"ping\"}\n");
        let v = t.recv().unwrap().unwrap();
        assert_eq!(v["method"], json!("ping"));
    }

    #[test]
    fn recv_reads_exactly_one_message_per_call_even_with_more_buffered() {
        let mut t = transport(
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"a\"}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"b\"}\n",
        );
        let first = t.recv().unwrap().unwrap();
        let second = t.recv().unwrap().unwrap();
        assert_eq!(first["method"], json!("a"));
        assert_eq!(second["method"], json!("b"));
    }

    #[test]
    fn recv_rejects_malformed_json() {
        let mut t = transport("not json at all\n");
        assert!(t.recv().is_err());
    }

    #[test]
    fn recv_handles_crlf_line_endings() {
        let mut t = transport("{\"jsonrpc\":\"2.0\",\"method\":\"x\"}\r\n");
        let v = t.recv().unwrap().unwrap();
        assert_eq!(v["method"], json!("x"));
    }

    #[test]
    fn recv_rejects_a_line_over_the_size_cap() {
        // One byte over the cap, no newline -- must not hang or OOM,
        // must error cleanly instead.
        let huge = "x".repeat(MAX_LINE_BYTES + 1);
        let mut t = transport(&huge);
        assert!(t.recv().is_err());
    }

    #[test]
    fn send_escapes_embedded_newlines_so_one_message_stays_one_line() {
        let mut t = LineTransport::new(Cursor::new(Vec::new()), Vec::new());
        t.send(&json!({"text": "line one\nline two"})).unwrap();
        let written = String::from_utf8(t.writer).unwrap();
        assert_eq!(
            written.matches('\n').count(),
            1,
            "must be exactly one real newline: the terminator"
        );
        assert!(written.contains("line one\\nline two"));
    }
}
