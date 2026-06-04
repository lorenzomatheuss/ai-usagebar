//! Budget guards — input token estimation, cost ceiling check, and 1/min
//! rate limiter (AC-5 of Story 1.15).
//!
//! These guards protect the user (and the project's free tier) from runaway
//! costs and prevent abusive request volumes. They run BEFORE any HTTP call,
//! so the network is never engaged on budget violations.
//!
//! ## Token estimation heuristic
//!
//! We use the well-known `chars / 4` approximation for English-leaning
//! tooluse JSON payloads. It's deliberately rough: the goal is to catch
//! gross overshoots (an 8000-token payload, ~32k chars), not to predict
//! tokens to within ±100. Real tokenization happens server-side; we just
//! refuse to send obviously-too-large requests.
//!
//! Alternative considered: `tiktoken-rs`. Rejected for v1.0 because (a) it
//! ships a 4MB BPE vocab embedded in the binary and (b) Anthropic uses a
//! different tokenizer than OpenAI's BPE, so accuracy gains are marginal.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::schema::{InsightsContext, InsightsError};

/// Maximum input tokens (estimated) we'll send to Anthropic. Above this, we
/// return `InsightsError::InputBudgetExceeded` without making the request.
///
/// Chosen to keep cost per insight under $0.05 at Sonnet pricing (current as
/// of 2026-Q2): 8000 input tokens × $3/1M + 1000 output tokens × $15/1M ≈
/// $0.04. The eval pipeline (Story 1.17) will refine this once we have
/// real-traffic distribution data.
pub const MAX_INPUT_TOKENS: u32 = 8000;

/// Rate limit window — 1 insight request per minute per `RateLimiter`
/// instance. Stored as `Duration` to make the arithmetic obvious at call
/// sites.
pub const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);

/// Estimate the number of input tokens needed to send the rendered prompt
/// + the JSON-serialized context to Anthropic.
///
/// Returns a rough upper bound (charset 4 chars ≈ 1 token). Callers should
/// treat this as advisory; the real tokenization happens on Anthropic's
/// side. The number is suitable for **gross overshoot detection only**,
/// which is the only use case we have in v1.0.
pub fn estimate_input_tokens(prompt: &str, context: &InsightsContext) -> u32 {
    let context_json = serde_json::to_string(context).unwrap_or_default();
    let total_chars = prompt.len() + context_json.len();
    // div_ceil to avoid undershoot on tiny payloads — better to overestimate
    // by 1 token than underestimate.
    total_chars.div_ceil(4) as u32
}

/// Check that an estimated request stays under the cost budget. Returns the
/// estimated USD cost on success, or `InputBudgetExceeded` on failure.
///
/// `model` is currently informational — we hardcode Sonnet pricing in v1.0.
/// When Story 1.17's eval pipeline runs against multiple models we'll
/// extend this to a `model -> price` map.
pub fn check_cost_budget(estimated_tokens: u32, _model: &str) -> Result<f64, InsightsError> {
    if estimated_tokens > MAX_INPUT_TOKENS {
        return Err(InsightsError::InputBudgetExceeded {
            tokens: estimated_tokens,
        });
    }
    // Sonnet input pricing: $3 / 1M tokens (USD, 2026-Q2).
    let estimated_cost_usd = (estimated_tokens as f64) * 3.0 / 1_000_000.0;
    Ok(estimated_cost_usd)
}

/// Per-instance 1/min rate limiter. Created once per `RealAnthropicClient`
/// and consulted before every streaming request.
///
/// Implementation: stores the timestamp of the last successful "go-ahead"
/// in a `Mutex<Option<Instant>>`. On `try_acquire`, if the previous
/// timestamp is within `RATE_LIMIT_WINDOW`, returns `RateLimit`. Otherwise
/// updates the timestamp and returns `Ok(())`.
pub struct RateLimiter {
    last_request: Mutex<Option<Instant>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            last_request: Mutex::new(None),
        }
    }

    /// Try to acquire a "go-ahead" for a new request. Returns
    /// `Err(InsightsError::RateLimit)` if called within
    /// `RATE_LIMIT_WINDOW` of the previous successful acquisition.
    pub fn try_acquire(&self) -> Result<(), InsightsError> {
        let mut last = self
            .last_request
            .lock()
            .map_err(|_| InsightsError::RateLimit("rate limiter mutex poisoned".to_string()))?;
        let now = Instant::now();
        if let Some(prev) = *last {
            let elapsed = now.duration_since(prev);
            if elapsed < RATE_LIMIT_WINDOW {
                let remaining = RATE_LIMIT_WINDOW - elapsed;
                return Err(InsightsError::RateLimit(format!(
                    "next request allowed in {}s",
                    remaining.as_secs() + 1
                )));
            }
        }
        *last = Some(now);
        Ok(())
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::insights::schema::VendorAggregate;

    fn small_ctx() -> InsightsContext {
        InsightsContext::new(vec![VendorAggregate {
            vendor_id: "anthropic".to_string(),
            account_id: Some("hash1234".to_string()),
            total_tokens: 1000,
            total_cost_usd: 0.05,
            window_days: 7,
            daily_tokens: vec![100; 7],
            daily_cost_usd: vec![0.005; 7],
            tag: None,
        }])
    }

    fn huge_ctx() -> InsightsContext {
        // Inflate the context so the JSON serialization blows past 32k chars
        // (≈8k tokens at chars/4) reliably.
        InsightsContext::new(
            (0..200)
                .map(|i| VendorAggregate {
                    vendor_id: format!("vendor{i}"),
                    account_id: Some("hash1234".to_string()),
                    total_tokens: 1_000_000,
                    total_cost_usd: 100.0,
                    window_days: 365,
                    daily_tokens: vec![1_000_000; 365],
                    daily_cost_usd: vec![100.0; 365],
                    tag: Some("a-long-tag-name-that-uses-many-characters".to_string()),
                })
                .collect(),
        )
    }

    #[test]
    fn estimate_input_tokens_returns_positive() {
        let ctx = small_ctx();
        let tokens = estimate_input_tokens("Analyze usage: {{usage_payload}}", &ctx);
        assert!(tokens > 0);
        // Small payload should be well under the budget.
        assert!(tokens < MAX_INPUT_TOKENS);
    }

    #[test]
    fn check_cost_budget_rejects_oversized_input() {
        let ctx = huge_ctx();
        let tokens = estimate_input_tokens("p", &ctx);
        assert!(tokens > MAX_INPUT_TOKENS, "huge_ctx must exceed budget");
        let err = check_cost_budget(tokens, "claude-3-5-sonnet-20241022").unwrap_err();
        match err {
            InsightsError::InputBudgetExceeded { tokens: t } => assert_eq!(t, tokens),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn check_cost_budget_accepts_small_input() {
        let cost = check_cost_budget(1000, "claude-3-5-sonnet-20241022").unwrap();
        assert!(cost > 0.0);
        assert!(cost < 0.01);
    }

    #[test]
    fn rate_limiter_blocks_second_request_within_window() {
        let rl = RateLimiter::new();
        rl.try_acquire().unwrap();
        let err = rl.try_acquire().unwrap_err();
        assert!(matches!(err, InsightsError::RateLimit(_)));
    }

    #[test]
    fn rate_limiter_starts_unblocked() {
        let rl = RateLimiter::new();
        assert!(rl.try_acquire().is_ok());
    }
}
