//! `RealAnthropicClient` — production HTTP streaming against Anthropic
//! Messages API in tool_use mode (ADR-7).
//!
//! ## Flow
//!
//! 1. Acquire rate-limit token (1/min/instance, AC-5).
//! 2. Redact context: hash account names via `sha256(vendor_id +
//!    account_name)[..8]` (PRD §6.5 privacy invariant).
//! 3. Estimate input tokens; reject if > [`budget::MAX_INPUT_TOKENS`].
//! 4. POST `messages` endpoint with `stream: true` and `tool_choice =
//!    "submit_insight"`.
//! 5. For each SSE event:
//!    - `content_block_delta` with `input_json_delta` → append to accumulator,
//!      fire `callback.on_token(chunk)`.
//!    - `message_stop` → break loop.
//!    - other → log + ignore (per Anthropic SSE spec, intermediate events
//!      like `ping`/`message_start` are advisory).
//! 6. Validate the accumulated JSON against the schema (`schema::validate_output`).
//! 7. Return `Ok(InsightsOutput)` as the future's value.
//!
//! ## Cancellation (AR-2 resolution §Decision 3)
//!
//! The chunk-pulling loop wraps `stream.next()` in `tokio::select!` against
//! `cancel_token.cancelled()`. If the cancel arm fires, the stream is
//! dropped (which closes the HTTP connection via reqwest) and the function
//! returns `Err(Cancelled)` without firing further callbacks.

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::budget::{RateLimiter, check_cost_budget, estimate_input_tokens};
use super::cancel::CancelHandle;
use super::llm_client::{InsightsCallback, LlmClient};
use super::schema::{InsightsContext, InsightsError, InsightsOutput, validate_output};

#[cfg(test)]
use super::schema::VendorAggregate;

/// Default Anthropic Messages endpoint. Overridable via
/// `RealAnthropicClient::new_with_base_url` for the AR-2 spike (mockito
/// server) and integration tests.
pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";

/// Model used in v1.0. ADR-7 selects Sonnet 3.5 (latest snapshot).
pub const DEFAULT_MODEL: &str = "claude-3-5-sonnet-20241022";

/// Anthropic API version header value. Required by every request to
/// `/v1/messages` — see Anthropic docs.
pub const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Production Anthropic client. Hold one instance per app launch; cloning
/// shares the underlying `reqwest::Client` connection pool but creates an
/// independent `RateLimiter` (intentional — if a caller wants to share the
/// rate limit, they share the `Arc<RealAnthropicClient>`).
pub struct RealAnthropicClient {
    http: HttpClient,
    base_url: String,
    api_key: String,
    rate_limiter: Arc<RateLimiter>,
}

impl RealAnthropicClient {
    /// Constructs a new client against the public Anthropic endpoint.
    pub fn new(api_key: String) -> Self {
        Self::new_with_base_url(api_key, DEFAULT_BASE_URL.to_string())
    }

    /// Constructs a client against a custom base URL. Used by the AR-2
    /// spike to point at a mockito server emitting deterministic SSE
    /// chunks.
    pub fn new_with_base_url(api_key: String, base_url: String) -> Self {
        let http = HttpClient::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("reqwest client build should not fail for default settings");
        Self {
            http,
            base_url,
            api_key,
            rate_limiter: Arc::new(RateLimiter::new()),
        }
    }

    /// SHA-256-hash account names in the context payload. Idempotent — if
    /// `account_id` already looks like an 8-char hash (i.e. the caller used
    /// `InsightsContext::synthetic` or pre-hashed via the history layer),
    /// we leave it alone. Otherwise we apply `sha256(vendor_id +
    /// account_name)[..8]`.
    pub(crate) fn redact_payload(&self, ctx: &InsightsContext) -> InsightsContext {
        let mut redacted = ctx.clone();
        for vendor in &mut redacted.usage_payload {
            if let Some(account_id) = &vendor.account_id {
                if account_id.len() != 8 || !account_id.chars().all(|c| c.is_ascii_hexdigit()) {
                    vendor.account_id = Some(hash_account(&vendor.vendor_id, account_id));
                }
            }
        }
        redacted
    }

    /// Render the prompt body. v1.0 uses an embedded template; future
    /// versions can swap in `prompts/insights.v1.md` at build time. The
    /// embedded template MUST stay in sync with `prompts/insights.v1.md`.
    fn render_prompt(&self, ctx: &InsightsContext) -> String {
        // Minimal v1 prompt template. Real version lives in
        // `prompts/insights.v1.md`; for v1.0 we embed it here to avoid
        // shipping the prompts directory in the macOS app bundle.
        format!(
            "Analyze the following LLM API usage payload across {} day(s) \
             and produce a structured insight via the `submit_insight` tool. \
             Usage payload: {}",
            ctx.since_days,
            serde_json::to_string(&ctx.usage_payload).unwrap_or_default()
        )
    }
}

#[async_trait]
impl LlmClient for RealAnthropicClient {
    async fn request_insight_streaming(
        &self,
        context: InsightsContext,
        callback: Arc<dyn InsightsCallback>,
        cancel: Arc<CancelHandle>,
    ) -> Result<InsightsOutput, InsightsError> {
        // 1. Rate limit (AC-5).
        self.rate_limiter.try_acquire()?;

        // 2. Redact privacy-sensitive fields.
        let redacted = self.redact_payload(&context);

        // 3. Render prompt and check budget.
        let prompt = self.render_prompt(&redacted);
        let tokens = estimate_input_tokens(&prompt, &redacted);
        check_cost_budget(tokens, DEFAULT_MODEL)?;

        // 4. Build request body. Anthropic Messages API tool_use mode.
        let body = AnthropicRequest {
            model: DEFAULT_MODEL.to_string(),
            max_tokens: 1000,
            stream: true,
            tool_choice: ToolChoice {
                type_: "tool".to_string(),
                name: "submit_insight".to_string(),
            },
            tools: vec![submit_insight_tool()],
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt,
            }],
        };

        let start = Instant::now();
        let url = format!("{}/v1/messages", self.base_url);
        let request = self
            .http
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body);

        // Race the response future itself against cancellation — Swift may
        // cancel even before the TCP handshake completes.
        let token = cancel.token();
        let response = tokio::select! {
            biased;
            _ = token.cancelled() => return Err(InsightsError::Cancelled),
            res = request.send() => match res {
                Ok(r) => r,
                Err(e) if e.is_timeout() => return Err(InsightsError::Timeout { seconds: 15 }),
                Err(e) => return Err(InsightsError::Network(e.to_string())),
            },
        };

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(if status.as_u16() == 401 {
                InsightsError::Auth(text)
            } else if status.as_u16() == 429 {
                InsightsError::RateLimit(text)
            } else {
                InsightsError::Network(format!("status={}: {}", status, text))
            });
        }

        // 5. Drain the SSE stream chunk-by-chunk, racing each chunk against
        //    cancellation.
        let mut stream = response.bytes_stream();
        let mut sse_buffer = String::new();
        let mut json_accumulator = String::new();

        loop {
            tokio::select! {
                biased;
                _ = token.cancelled() => {
                    // Drop the stream — this closes the underlying TCP
                    // connection and prevents further callback invocations.
                    drop(stream);
                    return Err(InsightsError::Cancelled);
                }
                chunk = stream.next() => match chunk {
                    Some(Ok(bytes)) => {
                        // Append bytes to the SSE buffer, parse complete
                        // event blocks (terminated by "\n\n"), and dispatch.
                        let text = std::str::from_utf8(&bytes)
                            .map_err(|e| InsightsError::ParseFailure(e.to_string()))?;
                        sse_buffer.push_str(text);
                        while let Some(end) = sse_buffer.find("\n\n") {
                            let event = sse_buffer[..end].to_string();
                            sse_buffer.drain(..end + 2);
                            if let Some(delta) = parse_input_json_delta(&event) {
                                callback.on_token(delta.clone());
                                json_accumulator.push_str(&delta);
                            }
                            if event.contains("event: message_stop") {
                                // Defensive: in practice the stream will
                                // close naturally on message_stop; this
                                // break is belt-and-suspenders.
                                break;
                            }
                        }
                    }
                    Some(Err(e)) => return Err(InsightsError::Network(e.to_string())),
                    None => break, // EOS
                }
            }
            // Re-check cancellation between iterations in case it was
            // signalled while we were inside the chunk-handling branch.
            if token.is_cancelled() {
                return Err(InsightsError::Cancelled);
            }
        }

        // 6. Validate accumulated JSON.
        let mut output = validate_output(&json_accumulator)?;
        output.latency_ms = start.elapsed().as_millis() as i64;
        output.prompt_version = context.prompt_version.clone();
        Ok(output)
    }
}

/// Produce a deterministic 8-character hex hash for `(vendor_id, account_name)`
/// — the canonical PRD §6.5 redaction.
fn hash_account(vendor_id: &str, account_name: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(vendor_id.as_bytes());
    hasher.update(b":");
    hasher.update(account_name.as_bytes());
    let digest = hasher.finalize();
    // Take the first 4 bytes → 8 hex characters. Collision probability for
    // a single user's account namespace (≤10s of accounts) is negligible.
    digest.iter().take(4).fold(String::new(), |mut acc, byte| {
        acc.push_str(&format!("{byte:02x}"));
        acc
    })
}

/// Try to extract the `input_json_delta` payload from one SSE event block.
/// Returns `None` for non-delta events (`ping`, `message_start`,
/// `content_block_start`, `message_stop`, ...).
fn parse_input_json_delta(event: &str) -> Option<String> {
    // Anthropic SSE format:
    //
    //   event: content_block_delta
    //   data: {"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"..."}}
    //
    // We only care about the `data:` line of `content_block_delta` events.
    if !event.contains("event: content_block_delta") {
        return None;
    }
    let data_line = event.lines().find(|l| l.starts_with("data:"))?;
    let payload = data_line.trim_start_matches("data:").trim();
    let parsed: serde_json::Value = serde_json::from_str(payload).ok()?;
    let delta_type = parsed.get("delta")?.get("type")?.as_str()?;
    if delta_type != "input_json_delta" {
        return None;
    }
    parsed
        .get("delta")?
        .get("partial_json")?
        .as_str()
        .map(|s| s.to_string())
}

fn submit_insight_tool() -> serde_json::Value {
    serde_json::json!({
        "name": "submit_insight",
        "description": "Submit a structured insight derived from the LLM API usage payload.",
        "input_schema": {
            "type": "object",
            "properties": {
                "headline": { "type": "string", "maxLength": 120 },
                "insights": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "enum": ["Trend", "Anomaly", "BudgetRisk", "OptimizationOpportunity"] },
                            "severity": { "type": "string", "enum": ["Info", "Warning", "Critical"] },
                            "message": { "type": "string" },
                            "evidence": { "type": "string" }
                        },
                        "required": ["type", "severity", "message", "evidence"]
                    }
                },
                "recommendation": { "type": "string", "maxLength": 280 },
                "cost_usd": { "type": "number" },
                "latency_ms": { "type": "integer" },
                "prompt_version": { "type": "string" }
            },
            "required": ["headline", "insights", "recommendation", "cost_usd", "latency_ms", "prompt_version"]
        }
    })
}

// ----- Anthropic request DTOs ------------------------------------------------

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    stream: bool,
    tool_choice: ToolChoice,
    tools: Vec<serde_json::Value>,
    messages: Vec<Message>,
}

#[derive(Serialize)]
struct ToolChoice {
    #[serde(rename = "type")]
    type_: String,
    name: String,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

// Helper visible for tests (live.rs may want to drive a real check).
#[derive(Deserialize)]
#[allow(dead_code)]
struct AnthropicErrorResponse {
    #[serde(rename = "type")]
    type_: String,
    error: serde_json::Value,
}

/// Holds an explicit reference to a `VendorAggregate` to ensure the
/// `redact_payload` round-trip preserves the non-account fields. Used as
/// an anchor for the privacy unit test.
#[cfg(test)]
fn assert_non_account_fields_equal(a: &VendorAggregate, b: &VendorAggregate) {
    assert_eq!(a.vendor_id, b.vendor_id);
    assert_eq!(a.total_tokens, b.total_tokens);
    assert_eq!(a.total_cost_usd, b.total_cost_usd);
    assert_eq!(a.window_days, b.window_days);
    assert_eq!(a.daily_tokens, b.daily_tokens);
    assert_eq!(a.daily_cost_usd, b.daily_cost_usd);
    assert_eq!(a.tag, b.tag);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_account_is_deterministic() {
        let a = hash_account("anthropic", "Acme Inc.");
        let b = hash_account("anthropic", "Acme Inc.");
        assert_eq!(a, b);
        assert_eq!(a.len(), 8);
    }

    #[test]
    fn hash_account_distinguishes_vendor_namespace() {
        let a = hash_account("anthropic", "name");
        let b = hash_account("openai", "name");
        assert_ne!(a, b, "vendor_id must namespace the hash");
    }

    #[test]
    fn redact_payload_hashes_plaintext_account_names() {
        let client = RealAnthropicClient::new("test-key".to_string());
        let ctx = InsightsContext::new(vec![VendorAggregate {
            vendor_id: "anthropic".to_string(),
            account_id: Some("Acme Inc.".to_string()),
            total_tokens: 1000,
            total_cost_usd: 0.5,
            window_days: 7,
            daily_tokens: vec![100; 7],
            daily_cost_usd: vec![0.07; 7],
            tag: None,
        }]);
        let redacted = client.redact_payload(&ctx);
        let r = &redacted.usage_payload[0];
        assert_ne!(r.account_id.as_deref(), Some("Acme Inc."));
        assert_eq!(r.account_id.as_deref().unwrap().len(), 8);
        // Non-account fields unchanged.
        assert_non_account_fields_equal(&ctx.usage_payload[0], r);
    }

    #[test]
    fn redact_payload_idempotent_for_already_hashed_ids() {
        let client = RealAnthropicClient::new("test-key".to_string());
        let ctx = InsightsContext::new(vec![VendorAggregate {
            vendor_id: "anthropic".to_string(),
            account_id: Some("deadbeef".to_string()), // 8 hex chars
            total_tokens: 1000,
            total_cost_usd: 0.5,
            window_days: 7,
            daily_tokens: vec![100; 7],
            daily_cost_usd: vec![0.07; 7],
            tag: None,
        }]);
        let redacted = client.redact_payload(&ctx);
        assert_eq!(
            redacted.usage_payload[0].account_id.as_deref(),
            Some("deadbeef")
        );
    }

    #[test]
    fn parse_input_json_delta_extracts_partial_json() {
        let event = "event: content_block_delta\n\
                     data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"headline\\\":\"}}";
        let delta = parse_input_json_delta(event).unwrap();
        assert_eq!(delta, "{\"headline\":");
    }

    #[test]
    fn parse_input_json_delta_ignores_other_events() {
        let event = "event: ping\ndata: {\"type\":\"ping\"}";
        assert!(parse_input_json_delta(event).is_none());
    }
}
