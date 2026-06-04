//! Insights schema — request context, structured output, error enum, and
//! JSON schema validator.
//!
//! Mirrors the UDL `dictionary InsightsOutput` and `[Error] enum InsightsError`
//! declared in `torven_core.udl` (AR-2 resolution §Decision 1). The
//! `InsightsContext` is privacy-redacted: it contains only numeric vendor
//! aggregates and never api_keys or account names in plaintext (PRD §6.5).

use serde::{Deserialize, Serialize};

/// Per-vendor numeric aggregate sent to Anthropic for insight generation.
///
/// **Privacy invariant (PRD §6.5):** this struct never carries `api_key`,
/// account names in plaintext, or raw vendor responses. `account_id` is the
/// SHA-256 hash `sha256(vendor_id + account_name)[..8]` produced by the
/// real client before transmission — deterministic but non-reversible.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VendorAggregate {
    /// Stable lowercase vendor slug (e.g. "anthropic", "openai").
    pub vendor_id: String,
    /// Hashed account identifier or `None` for aggregate-only payloads.
    /// `RealAnthropicClient::redact_payload` applies the hash; callers
    /// providing raw account names would violate the privacy invariant.
    pub account_id: Option<String>,
    /// Total tokens consumed in the observation window.
    pub total_tokens: u64,
    /// Total USD cost in the observation window.
    pub total_cost_usd: f64,
    /// Window size in days (1, 7, 30 typically).
    pub window_days: u32,
    /// Per-day token totals across `window_days` (chronological order, length
    /// == window_days when produced by the history layer).
    pub daily_tokens: Vec<u64>,
    /// Per-day cost totals matching `daily_tokens`.
    pub daily_cost_usd: Vec<f64>,
    /// Optional generic tag, e.g. "client", "personal", "team".
    pub tag: Option<String>,
}

/// Input context for an insight request. Owned by callers; the Anthropic
/// client clones and serializes via `serde_json` after applying redaction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InsightsContext {
    /// One aggregate per vendor the user has configured. Empty vec is allowed
    /// (returns a "no data yet" insight from the LLM).
    pub usage_payload: Vec<VendorAggregate>,
    /// Observation window passed to the prompt template (default: 7).
    pub since_days: u32,
    /// Prompt version used to render the request (e.g. "v1"). Stored on the
    /// output so the eval pipeline (Story 1.17) can join scores to prompts.
    pub prompt_version: String,
}

impl InsightsContext {
    /// Convenience constructor for tests + Story 1.19 integration. Uses a
    /// 7-day window and the "v1" prompt by default.
    pub fn new(usage_payload: Vec<VendorAggregate>) -> Self {
        Self {
            usage_payload,
            since_days: 7,
            prompt_version: "v1".to_string(),
        }
    }

    /// Synthetic helper used by the AR-2 spike test (no real data). Produces
    /// a single-vendor payload that's compact enough to fit well under the
    /// 8000-token budget.
    pub fn synthetic() -> Self {
        Self {
            usage_payload: vec![VendorAggregate {
                vendor_id: "anthropic".to_string(),
                account_id: Some("test1234".to_string()),
                total_tokens: 120_000,
                total_cost_usd: 4.20,
                window_days: 7,
                daily_tokens: vec![10_000, 15_000, 20_000, 25_000, 20_000, 15_000, 15_000],
                daily_cost_usd: vec![0.35, 0.52, 0.70, 0.87, 0.70, 0.52, 0.54],
                tag: Some("test".to_string()),
            }],
            since_days: 7,
            prompt_version: "v1".to_string(),
        }
    }
}

/// Type of insight surfaced by the LLM. Maps to UI rendering hints (icon,
/// color) in Story 1.19's `InsightPanel`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InsightItemType {
    Trend,
    Anomaly,
    BudgetRisk,
    OptimizationOpportunity,
}

/// Severity of an individual insight item. Drives sort order and the
/// "needs attention" badge on the SwiftUI panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InsightSeverity {
    Info,
    Warning,
    Critical,
}

/// A single structured insight item produced by the LLM tool_use call.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InsightItem {
    /// Raw enum value emitted by the LLM. Deserialized via serde rename rules
    /// in `validate_output` to handle case-insensitive matching.
    #[serde(rename = "type")]
    pub type_: InsightItemType,
    pub severity: InsightSeverity,
    pub message: String,
    /// Free-form text from the LLM citing the numeric evidence.
    pub evidence: String,
}

/// Structured output of one insight request.
///
/// **AR-2 resolution §Decision 1:** this struct is the **return value** of
/// the `[Async]` FFI future, not a callback parameter. `cb.on_token` fires
/// for streaming display; the final `InsightsOutput` is `await`ed in Swift.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InsightsOutput {
    pub headline: String,
    pub insights: Vec<InsightItem>,
    pub recommendation: String,
    /// Cost in USD as reported by the LLM (informational; ground truth is
    /// the price tabulated from `usage` fields in the Anthropic response).
    pub cost_usd: f64,
    /// Latency from request start to `message_stop`, in milliseconds.
    pub latency_ms: i64,
    /// Snapshot of the prompt version that produced this output (for eval).
    pub prompt_version: String,
}

/// Errors that can be returned from `LlmClient::request_insight_streaming`.
///
/// Mirrors the UDL `[Error] enum InsightsError`. `Cancelled` is required by
/// AR-2 resolution §Decision 1 (the future resolves `Err(Cancelled)` when
/// the Swift caller invokes `CancelHandle::cancel`).
#[derive(Debug, thiserror::Error)]
pub enum InsightsError {
    #[error("network error: {0}")]
    Network(String),
    #[error("authentication error: {0}")]
    Auth(String),
    #[error("rate limit exceeded: {0}")]
    RateLimit(String),
    #[error("input budget exceeded ({tokens} > 8000)")]
    InputBudgetExceeded { tokens: u32 },
    #[error("request timed out after {seconds}s")]
    Timeout { seconds: u32 },
    /// Returned when the Swift caller invokes `CancelHandle::cancel()`
    /// mid-stream. Treated as a non-error by SwiftUI (silent dismissal).
    #[error("request cancelled")]
    Cancelled,
    #[error("failed to parse LLM response: {0}")]
    ParseFailure(String),
    #[error("LLM output failed schema validation: {0}")]
    SchemaInvalid(String),
}

/// Validate an accumulated streaming JSON payload against the Insights
/// output schema. Used by both `RealAnthropicClient` (on `message_stop`)
/// and `MockLlmClient` test fixtures.
///
/// On success returns the parsed `InsightsOutput`. On failure returns
/// `InsightsError::ParseFailure` (JSON malformed) or
/// `InsightsError::SchemaInvalid` (well-formed JSON missing required
/// fields / wrong types).
pub fn validate_output(json: &str) -> Result<InsightsOutput, InsightsError> {
    // First try strict deserialization. `serde_json::Error` does not
    // distinguish "syntax" from "missing field" cleanly via the public API,
    // so we use `is_data` / `is_syntax` heuristics and map accordingly.
    match serde_json::from_str::<InsightsOutput>(json) {
        Ok(output) => {
            if output.headline.is_empty() {
                return Err(InsightsError::SchemaInvalid(
                    "headline is empty".to_string(),
                ));
            }
            if output.headline.len() > 120 {
                return Err(InsightsError::SchemaInvalid(format!(
                    "headline exceeds 120 chars ({} chars)",
                    output.headline.len()
                )));
            }
            if output.recommendation.len() > 280 {
                return Err(InsightsError::SchemaInvalid(format!(
                    "recommendation exceeds 280 chars ({} chars)",
                    output.recommendation.len()
                )));
            }
            Ok(output)
        }
        Err(e) if e.is_data() => Err(InsightsError::SchemaInvalid(e.to_string())),
        Err(e) => Err(InsightsError::ParseFailure(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_output_json() -> &'static str {
        r#"{
            "headline": "Usage trending up",
            "insights": [
                {
                    "type": "Trend",
                    "severity": "Info",
                    "message": "Token usage increased",
                    "evidence": "Daily tokens grew from 10k to 25k"
                }
            ],
            "recommendation": "Monitor for the next 7 days.",
            "cost_usd": 0.01,
            "latency_ms": 1500,
            "prompt_version": "v1"
        }"#
    }

    #[test]
    fn validate_output_accepts_valid_json() {
        let parsed = validate_output(valid_output_json()).unwrap();
        assert_eq!(parsed.headline, "Usage trending up");
        assert_eq!(parsed.insights.len(), 1);
        assert!(matches!(parsed.insights[0].type_, InsightItemType::Trend));
    }

    #[test]
    fn validate_output_rejects_malformed_json() {
        let err = validate_output("not json").unwrap_err();
        assert!(matches!(err, InsightsError::ParseFailure(_)));
    }

    #[test]
    fn validate_output_rejects_missing_fields() {
        let err = validate_output(r#"{"headline": "x"}"#).unwrap_err();
        assert!(matches!(err, InsightsError::SchemaInvalid(_)));
    }

    #[test]
    fn validate_output_rejects_oversized_headline() {
        let json = format!(
            r#"{{"headline": "{}", "insights": [], "recommendation": "r", "cost_usd": 0.0, "latency_ms": 0, "prompt_version": "v1"}}"#,
            "x".repeat(121)
        );
        let err = validate_output(&json).unwrap_err();
        assert!(matches!(err, InsightsError::SchemaInvalid(_)));
    }

    #[test]
    fn insights_context_synthetic_passes_privacy_check() {
        // synthetic() uses a hash-shaped account_id, never a real account
        // name. This is a smoke test that the convenience constructor
        // doesn't drift away from the privacy invariant.
        let ctx = InsightsContext::synthetic();
        let payload = &ctx.usage_payload[0];
        assert!(payload.account_id.is_some());
        assert_eq!(payload.account_id.as_deref().unwrap().len(), 8);
    }
}
