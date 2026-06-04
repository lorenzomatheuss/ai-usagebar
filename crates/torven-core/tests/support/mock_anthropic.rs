//! In-process Anthropic Messages mock — hand-rolled HTTP/1.1 server that
//! emits a deterministic, paced SSE stream of `content_block_delta` events
//! followed by `message_stop`.
//!
//! ## Why hand-rolled (not mockito)
//!
//! The AR-2 spike (Story 1.15 T9) needs **inter-chunk delay** so the
//! cancellation race is measurable. mockito 1.x can return chunked
//! bodies but does not pace chunks over time — they're all flushed
//! together once the body builder produces them. A tiny `tokio::net::
//! TcpListener` that writes one SSE event, sleeps, writes the next, etc.,
//! is the simplest way to get a real streaming response.
//!
//! ## Wire format produced
//!
//! ```
//! HTTP/1.1 200 OK\r\n
//! content-type: text/event-stream\r\n
//! transfer-encoding: chunked\r\n
//! \r\n
//! {chunk1}\r\n          ← chunked encoding
//! event: content_block_delta\n
//! data: {"type":"content_block_delta",...,"delta":{"type":"input_json_delta","partial_json":"..."}}\n\n
//! \r\n
//! ... (repeats N times) ...
//! {chunk_last}\r\n
//! event: message_stop\n
//! data: {"type":"message_stop"}\n\n
//! \r\n
//! 0\r\n                  ← chunked transfer end
//! \r\n
//! ```
//!
//! The accumulated `partial_json` chunks produce a complete, valid
//! `InsightsOutput` when concatenated.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

/// One running mock Anthropic server. Drop the handle to stop accepting
/// new connections; in-flight connections finish their natural lifecycle.
pub struct MockAnthropic {
    base_url: String,
    #[allow(dead_code)] // surfaced via accepts_remaining(); some tests don't need it.
    accepts_remaining: Arc<AtomicU64>,
    _handle: JoinHandle<()>,
}

impl MockAnthropic {
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Number of accept()s the server is still willing to handle. Useful
    /// in long-running spike loops — when this hits zero we know we
    /// exhausted the configured budget rather than the test running off
    /// the end of the loop.
    #[allow(dead_code)] // not consumed by every integration test
    pub fn accepts_remaining(&self) -> u64 {
        self.accepts_remaining.load(Ordering::SeqCst)
    }
}

/// Start a mock server that, for every accepted connection, emits
/// `chunk_count` `content_block_delta` events spaced `chunk_delay` apart,
/// followed by a `message_stop`. The accumulated `partial_json` content
/// across all chunks deserializes to a valid `InsightsOutput`.
///
/// `max_connections` bounds the number of concurrent client requests the
/// server will entertain — set high (>= cycles count) for AR-2 spike runs.
pub async fn start_streaming(
    chunk_count: usize,
    chunk_delay: Duration,
    max_connections: u64,
) -> MockAnthropic {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);
    let accepts_remaining = Arc::new(AtomicU64::new(max_connections));

    let accepts_clone = accepts_remaining.clone();
    let handle = tokio::spawn(async move {
        loop {
            let (mut socket, _) = match listener.accept().await {
                Ok(s) => s,
                Err(_) => break,
            };
            if accepts_clone.fetch_sub(1, Ordering::SeqCst) == 0 {
                break;
            }
            tokio::spawn(async move {
                // Read enough of the request to consume the headers + body.
                // We don't validate — the Anthropic-side correctness is
                // exercised against the real API. For the mock, we just
                // wait until the client stops writing, then start streaming
                // our deterministic response.
                let mut buf = [0u8; 4096];
                let _ =
                    tokio::time::timeout(Duration::from_millis(100), socket.read(&mut buf)).await;

                let _ = socket
                    .write_all(
                        b"HTTP/1.1 200 OK\r\n\
                          content-type: text/event-stream\r\n\
                          transfer-encoding: chunked\r\n\
                          \r\n",
                    )
                    .await;

                // Generate `chunk_count` SSE events whose payloads
                // concatenate into a valid InsightsOutput. The first
                // chunk opens the JSON; the last closes it.
                let chunks = build_chunks(chunk_count);

                for chunk in &chunks {
                    let event = format!(
                        "event: content_block_delta\n\
                         data: {{\"type\":\"content_block_delta\",\"index\":0,\
                         \"delta\":{{\"type\":\"input_json_delta\",\"partial_json\":{}}}}}\n\n",
                        serde_json::to_string(chunk).unwrap_or_default(),
                    );
                    // Write the chunked-encoding header + body for this event.
                    let chunked = format!("{:x}\r\n{}\r\n", event.len(), event);
                    if socket.write_all(chunked.as_bytes()).await.is_err() {
                        return; // client dropped (cancelled) — fine.
                    }
                    tokio::time::sleep(chunk_delay).await;
                }

                let stop = "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
                let chunked = format!("{:x}\r\n{}\r\n", stop.len(), stop);
                let _ = socket.write_all(chunked.as_bytes()).await;
                let _ = socket.write_all(b"0\r\n\r\n").await;
            });
        }
    });

    MockAnthropic {
        base_url,
        accepts_remaining,
        _handle: handle,
    }
}

/// Build `n` chunks of `partial_json` strings whose concatenation yields
/// a complete `InsightsOutput`. The split is deterministic but arbitrary
/// — what matters for the spike is that we emit ≥30 distinct chunks on a
/// happy path (assertion 5 in the AR-2 spike).
fn build_chunks(n: usize) -> Vec<String> {
    // Whole JSON we want to produce by the end. Keep it small + valid.
    let full = r#"{"headline":"Usage trending up","insights":[{"type":"Trend","severity":"Info","message":"Token usage increased","evidence":"Daily tokens grew from 10k to 25k"}],"recommendation":"Monitor for the next 7 days.","cost_usd":0.012,"latency_ms":0,"prompt_version":"v1"}"#;
    if n == 0 {
        return vec![];
    }
    // Split into roughly equal-length slices on character boundaries.
    let bytes = full.as_bytes();
    let chunk_size = bytes.len().div_ceil(n);
    let mut out = Vec::with_capacity(n);
    let mut i = 0;
    while i < bytes.len() {
        let end = (i + chunk_size).min(bytes.len());
        out.push(String::from_utf8_lossy(&bytes[i..end]).to_string());
        i = end;
    }
    out
}

#[allow(dead_code)]
pub fn full_expected_json() -> &'static str {
    r#"{"headline":"Usage trending up","insights":[{"type":"Trend","severity":"Info","message":"Token usage increased","evidence":"Daily tokens grew from 10k to 25k"}],"recommendation":"Monitor for the next 7 days.","cost_usd":0.012,"latency_ms":0,"prompt_version":"v1"}"#
}
