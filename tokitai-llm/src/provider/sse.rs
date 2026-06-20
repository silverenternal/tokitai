//! T-044: shared Server-Sent Events (SSE) parser used by the
//! OpenAI / Anthropic / Ollama streaming providers.
//!
//! Wire format: an SSE stream is a sequence of records, each
//! terminated by a blank line. Every record is a list of
//! `field: value` lines; the only field we care about is
//! `data:` (the other fields — `event:`, `id:`, `retry:` — are
//! ignored). The literal string `[DONE]` is a sentinel that the
//! OpenAI / Anthropic streams emit to signal end-of-stream and is
//! surfaced to the caller as `Some(None)` so the provider can
//! decide how to handle it.
//!
//! The parser is intentionally tiny: it operates on a `&[u8]`
//! input, returns the JSON payload of every record, and a final
//! `true` when the `[DONE]` sentinel was seen. Chunk boundaries
//! that fall inside a JSON payload are tolerated (the chunk
//! buffer is carried across calls), so the provider can wrap it
//! in a `reqwest::Response::bytes_stream` and yield events as
//! they arrive without losing the tail of the last record.

/// Outcome of a single parse step. The provider loops on
/// `ParserState::feed` until the response stream is exhausted.
#[derive(Debug, Default, Clone)]
pub struct ParserState {
    /// Bytes carried across calls. A record may straddle two
    /// chunks; we accumulate the partial trailing record here and
    /// prepend it to the next call.
    buffer: Vec<u8>,
}

impl ParserState {
    /// Build an empty parser state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed one chunk of bytes. Returns the JSON payload of every
    /// complete record in the chunk and `Some(true)` when the
    /// final record carried the `[DONE]` sentinel. The state
    /// retains the partial trailing record (if any) for the next
    /// call.
    pub fn feed(&mut self, chunk: &[u8]) -> anyhow::Result<Vec<FeedResult>> {
        self.buffer.extend_from_slice(chunk);
        let mut out = Vec::new();
        loop {
            // Look for the record terminator (a blank line,
            // i.e. "\n\n" or "\r\n\r\n"). The OpenAI stream uses
            // "\n\n"; some proxies use "\r\n"; we accept both.
            let term = find_record_terminator(&self.buffer);
            let Some(end) = term else {
                break;
            };
            // Split off the record (everything up to and not
            // including the terminator) and parse it.
            let record: Vec<u8> = self.buffer.drain(..end).collect();
            // Skip the terminator (it is 2 bytes — either "\n\n"
            // or "\r\n\r\n").
            let term_len = record_terminator_len(&record, end);
            self.buffer.drain(..term_len);
            // The record's `data:` field is the JSON payload we
            // hand to the provider. The terminator is consumed
            // above; the record slice we still hold has had the
            // terminator stripped, so we hand that to the parser.
            let parsed = parse_sse_record(&record);
            out.push(parsed);
        }
        Ok(out)
    }

    /// True when the buffer still holds a partial record (no
    /// terminator was seen on the last `feed` call). Providers
    /// typically call `feed` until the response stream returns
    /// `None` and then call `flush` to handle any trailing bytes.
    pub fn has_partial(&self) -> bool {
        !self.buffer.is_empty()
    }

    /// Flush any partial record left in the buffer. The OpenAI
    /// / Anthropic streams are required to terminate with a
    /// blank line (the `[DONE]` sentinel is followed by one),
    /// so a non-empty buffer here usually indicates a truncated
    /// response. We still parse what we can so a missing
    /// terminator does not lose the final payload.
    pub fn flush(&mut self) -> anyhow::Result<Vec<FeedResult>> {
        if self.buffer.is_empty() {
            return Ok(Vec::new());
        }
        let record: Vec<u8> = self.buffer.drain(..).collect();
        Ok(vec![parse_sse_record(&record)])
    }
}

/// Outcome of parsing a single SSE record. `done` is `true`
/// when the record carried the `[DONE]` sentinel; in that case
/// `data` is `None` and the caller should stop iterating. The
/// `event` field carries the value of the `event:` line when
/// present (the OpenAI stream omits `event:`; the Anthropic
/// stream uses named events like `content_block_delta`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedResult {
    /// Event name (value of the `event:` line). `None` when
    /// the record did not carry an `event:` field.
    pub event: Option<String>,
    /// JSON payload of the record. `None` when `done == true`.
    pub data: Option<String>,
    /// `true` when the record carried the `[DONE]` sentinel.
    pub done: bool,
}

/// Locate the byte offset of the first record terminator in
/// `buf`. Returns `None` when no terminator is present. The
/// terminator is the first occurrence of either `"\n\n"` or
/// `"\r\n\r\n"`.
fn find_record_terminator(buf: &[u8]) -> Option<usize> {
    for i in 0..buf.len().saturating_sub(1) {
        if buf[i] == b'\n' && buf[i + 1] == b'\n' {
            return Some(i);
        }
    }
    if buf.len() >= 4 {
        for i in 0..buf.len() - 3 {
            if &buf[i..i + 4] == b"\r\n\r\n" {
                return Some(i);
            }
        }
    }
    None
}

/// Compute the length of the terminator that ended the record at
/// `end`. We need this to know how many bytes to drop after
/// `drain(..end)`. Both `"\n\n"` and `"\r\n\r\n"` are 2 bytes of
/// payload (the second `\n` is the start of the next record's
/// blank line); the helper returns 2 in both cases because
/// `find_record_terminator` returns the offset of the FIRST `\n`.
fn record_terminator_len(_record: &[u8], _end: usize) -> usize {
    // `find_record_terminator` returns the offset of the first
    // `\n` of the terminator. The terminator is 2 bytes long
    // regardless of the CRLF / LF flavour.
    2
}

/// Parse a single SSE record. Walks the byte slice line by line
/// (LF or CRLF separated), picks out the `data:` and `event:`
/// fields, and concatenates multiple `data:` values with
/// newlines (an SSE record may carry more than one `data:` line;
/// the value is the newline-joined string).
fn parse_sse_record(record: &[u8]) -> FeedResult {
    let text = String::from_utf8_lossy(record);
    let mut data_lines: Vec<&str> = Vec::new();
    let mut event_name: Option<&str> = None;
    for line in text.lines() {
        // An SSE line is `field: value` or `field:value`. A
        // single space after the colon is conventional; some
        // proxies emit `data:foo` (no space) — we accept both.
        let (field, value) = match line.find(':') {
            Some(idx) => (&line[..idx], line[idx + 1..].trim_start()),
            None => continue,
        };
        if field == "data" {
            data_lines.push(value);
        } else if field == "event" {
            event_name = Some(value);
        }
    }
    let joined = data_lines.join("\n");
    if joined == "[DONE]" {
        return FeedResult {
            event: event_name.map(|s| s.to_string()),
            data: None,
            done: true,
        };
    }
    if joined.is_empty() {
        // No `data:` line — could be a comment, a
        // `event:`-only dispatch marker, or a heartbeat. We
        // surface an empty payload so the provider can
        // decide to skip it.
        return FeedResult {
            event: event_name.map(|s| s.to_string()),
            data: Some(String::new()),
            done: false,
        };
    }
    FeedResult {
        event: event_name.map(|s| s.to_string()),
        data: Some(joined),
        done: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_data_record() {
        let mut s = ParserState::new();
        let out = s.feed(b"data: {\"a\":1}\n\n").unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data.as_deref(), Some("{\"a\":1}"));
        assert!(!out[0].done);
    }

    #[test]
    fn parses_done_sentinel() {
        let mut s = ParserState::new();
        let out = s.feed(b"data: [DONE]\n\n").unwrap();
        assert_eq!(out.len(), 1);
        assert!(out[0].done);
        assert!(out[0].data.is_none());
    }

    #[test]
    fn parses_multiple_records_in_one_chunk() {
        let mut s = ParserState::new();
        let out = s.feed(b"data: {\"a\":1}\n\ndata: {\"b\":2}\n\n").unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].data.as_deref(), Some("{\"a\":1}"));
        assert_eq!(out[1].data.as_deref(), Some("{\"b\":2}"));
    }

    #[test]
    fn splits_records_across_chunks() {
        let mut s = ParserState::new();
        let out1 = s.feed(b"data: {\"a\":").unwrap();
        assert!(out1.is_empty());
        let out2 = s.feed(b"1}\n\ndata: {\"b\":2}\n\n").unwrap();
        assert_eq!(out2.len(), 2);
        assert_eq!(out2[0].data.as_deref(), Some("{\"a\":1}"));
        assert_eq!(out2[1].data.as_deref(), Some("{\"b\":2}"));
    }

    #[test]
    fn parses_crlf_terminator() {
        let mut s = ParserState::new();
        let out = s.feed(b"data: {\"x\":1}\r\n\r\n").unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data.as_deref(), Some("{\"x\":1}"));
    }

    #[test]
    fn ignores_event_field() {
        let mut s = ParserState::new();
        let out = s.feed(b"event: message\ndata: {\"y\":1}\n\n").unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data.as_deref(), Some("{\"y\":1}"));
    }

    #[test]
    fn flush_returns_partial_record() {
        let mut s = ParserState::new();
        s.feed(b"data: {\"z\":").unwrap();
        assert!(s.has_partial());
        let out = s.flush().unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data.as_deref(), Some("{\"z\":"));
    }
}
