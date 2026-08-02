//! Shared SSE byte parser.
//!
//! The three protocols laipe supports ship slightly different SSE shapes:
//! - **OpenAI Chat Completions** — `data: {json}\n\n` only, with `data: [DONE]`
//!   to terminate. The event name is implicit.
//! - **OpenAI Responses** — `event: response.output_item.added\ndata: {json}\n\n`.
//!   The event name is **explicit** and the protocol is identified by it.
//! - **Anthropic Messages** — `event: content_block_delta\ndata: {json}\n\n`.
//!   Same shape as Responses, different event vocabulary.
//!
//! `SseParser` accepts all three; the per-protocol implementation picks out
//! the events it cares about.

use bytes::Bytes;
use serde_json::Value;

/// One logical frame the upstream sent.
#[derive(Debug, Clone)]
pub enum SseFrame {
    /// `data: {json}\n\n` — Chat Completions chunk.
    Data(Value),
    /// `data: [DONE]\n\n` — Chat Completions terminator.
    Done,
    /// `event: <name>\ndata: {json}\n\n` — Responses / Anthropic frame.
    Named { event: String, data: Value },
    /// `: ping\n\n` or other unparseable / ignored frame.
    Skip,
}

/// Minimal SSE byte-accumulator. Yields frames as they complete.
pub struct SseParser {
    buf: Vec<u8>,
}

impl SseParser {
    pub fn new() -> Self {
        Self {
            buf: Vec::with_capacity(4096),
        }
    }

    /// Feed a chunk of bytes, drain any complete frames, return them.
    pub fn feed(&mut self, bytes: &Bytes) -> Vec<SseFrame> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();

        while let Some(end) = find_double_newline(&self.buf) {
            let frame = self.buf.drain(..end).collect::<Vec<u8>>();
            // Drop the terminator (\n\n or \r\n\r\n).
            if self.buf.starts_with(b"\n\n") {
                self.buf.drain(..2);
            } else if self.buf.starts_with(b"\r\n\r\n") {
                self.buf.drain(..4);
            } else if self.buf.starts_with(b"\r\n") {
                // \r\n before \n\n (Windows-style: \r\n\r\n = end of one frame + \n\n terminator? no — just \r\n at start of next frame)
                // No-op: leave for next iteration
            }

            if let Some(item) = parse_frame(&frame) {
                out.push(item);
            }
        }

        out
    }
}

impl Default for SseParser {
    fn default() -> Self {
        Self::new()
    }
}

fn find_double_newline(buf: &[u8]) -> Option<usize> {
    (0..buf.len().saturating_sub(1)).find(|&i| buf[i] == b'\n' && buf[i + 1] == b'\n')
}

fn parse_frame(frame: &[u8]) -> Option<SseFrame> {
    let s = std::str::from_utf8(frame).ok()?;

    let mut event_name: Option<String> = None;
    let mut data_lines: Vec<&str> = Vec::new();

    for line in s.split('\n') {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        // Comment / heartbeat lines start with `:`.
        if line.starts_with(':') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("event:") {
            event_name = Some(rest.trim().to_string());
            continue;
        }
        if let Some(rest) = line.strip_prefix("data:") {
            let rest = rest.trim_start();
            if rest == "[DONE]" {
                return Some(SseFrame::Done);
            }
            data_lines.push(rest);
        }
        // ignore id: / retry: / unknown fields
    }

    if data_lines.is_empty() {
        return None;
    }

    let joined = data_lines.join("\n");
    let value: Value = match serde_json::from_str(&joined) {
        Ok(v) => v,
        Err(_) => return Some(SseFrame::Skip),
    };

    if let Some(name) = event_name {
        Some(SseFrame::Named {
            event: name,
            data: value,
        })
    } else {
        Some(SseFrame::Data(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_data_only() {
        let mut p = SseParser::new();
        let bytes = Bytes::from_static(b"data: {\"a\":1}\n\n");
        let out = p.feed(&bytes);
        assert_eq!(out.len(), 1);
        match &out[0] {
            SseFrame::Data(v) => assert_eq!(v["a"], 1),
            _ => panic!(),
        }
    }

    #[test]
    fn parses_named_event() {
        let mut p = SseParser::new();
        let bytes =
            Bytes::from_static(b"event: response.output_text.delta\ndata: {\"delta\":\"hi\"}\n\n");
        let out = p.feed(&bytes);
        assert_eq!(out.len(), 1);
        match &out[0] {
            SseFrame::Named { event, data } => {
                assert_eq!(event, "response.output_text.delta");
                assert_eq!(data["delta"], "hi");
            }
            _ => panic!(),
        }
    }

    #[test]
    fn parses_done_sentinel() {
        let mut p = SseParser::new();
        let bytes = Bytes::from_static(b"data: [DONE]\n\n");
        let out = p.feed(&bytes);
        assert!(matches!(out[0], SseFrame::Done));
    }

    #[test]
    fn handles_split_chunk() {
        let mut p = SseParser::new();
        let b1 = Bytes::from_static(b"data: {\"a\"");
        let b2 = Bytes::from_static(b":1}\n\n");
        assert!(p.feed(&b1).is_empty());
        let out = p.feed(&b2);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn skips_heartbeat() {
        let mut p = SseParser::new();
        let bytes = Bytes::from_static(b": ping\n\n");
        let out = p.feed(&bytes);
        assert!(out.is_empty());
    }
}
