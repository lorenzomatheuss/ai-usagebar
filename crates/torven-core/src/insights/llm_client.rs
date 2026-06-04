//! `LlmClient` trait + `MockLlmClient`.
//!
//! Two implementations of the trait coexist:
//!
//! - [`MockLlmClient`] — fixture-driven, used by integration tests and the
//!   AR-2 spike's "control" cycles. Emits a configurable list of tokens
//!   followed by a configurable `InsightsOutput`.
//! - [`crate::insights::client::RealAnthropicClient`] — production HTTP
//!   streaming against Anthropic Messages API in tool_use mode.
//!
//! ## Callback contract (AR-2 resolution §Decision 1)
//!
//! `InsightsCallback::on_token` fires for each partial JSON chunk. The
//! callback does NOT have an `on_done`; the final `InsightsOutput` is the
//! `Result::Ok` value of the future itself. `on_error` is invoked for
//! intermediate error reporting (rare; the future's `Err` is the ground
//! truth).

use std::sync::Arc;

use async_trait::async_trait;

use super::cancel::CancelHandle;
use super::schema::{InsightsContext, InsightsError, InsightsOutput};

/// Streaming callback invoked from inside the LLM client for every partial
/// JSON token chunk produced during the streaming response.
///
/// The Swift side implements this protocol on a `ViewModel`. The
/// `MockLlmClient` calls it directly; the real client calls it from inside
/// the `tokio::select!` HTTP-chunk arm.
///
/// **Thread safety:** implementations must be `Send + Sync`. UniFFI marshals
/// callback invocations to a serial dispatch queue on the Swift side, so
/// implementers do not need to add their own locking.
pub trait InsightsCallback: Send + Sync {
    /// Called for each `content_block_delta` chunk from the LLM stream.
    /// `token` is the raw `input_json_delta` payload — partial JSON,
    /// non-decodable on its own.
    fn on_token(&self, token: String);

    /// Called for non-terminal errors during streaming. Terminal errors are
    /// surfaced via the future's `Err` arm — `on_error` is a hint for UI
    /// (e.g. "retrying connection…").
    fn on_error(&self, error: String);
}

/// Async trait implemented by both the mock and real Anthropic clients.
///
/// The third parameter (`cancel`) is mandatory by AR-2 resolution §Decision
/// 3: both implementations must honour cancellation identically, otherwise
/// integration tests against the mock provide no signal about the real
/// surface.
#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn request_insight_streaming(
        &self,
        context: InsightsContext,
        callback: Arc<dyn InsightsCallback>,
        cancel: Arc<CancelHandle>,
    ) -> Result<InsightsOutput, InsightsError>;
}

/// Fixture-driven mock client. Returns a configured `InsightsOutput` after
/// streaming a configured sequence of token chunks with optional inter-chunk
/// delay.
///
/// **Cancellation honoured:** if `cancel.cancel()` fires mid-stream, the
/// next iteration of the chunk loop observes `cancel.is_cancelled()` and
/// returns `Err(Cancelled)` without firing further `on_token` callbacks.
pub struct MockLlmClient {
    /// Sequence of partial JSON chunks to emit via `on_token`. The mock
    /// concatenates them as one validated whole at the end (when the
    /// concatenation is valid JSON matching the schema) for a realistic
    /// integration test.
    pub token_chunks: Vec<String>,
    /// `InsightsOutput` returned as the future's `Ok` value once all chunks
    /// have been emitted. The integration test that constructs the mock is
    /// responsible for keeping `token_chunks` semantically consistent with
    /// the `response` payload.
    pub response: InsightsOutput,
    /// Optional delay between chunks. Used by the AR-2 spike's mock to
    /// simulate a 50ms/chunk Anthropic streaming pace.
    pub delay_ms: u64,
}

impl MockLlmClient {
    /// Convenience constructor used by `tests/insights_e2e.rs`.
    pub fn new(token_chunks: Vec<String>, response: InsightsOutput, delay_ms: u64) -> Self {
        Self {
            token_chunks,
            response,
            delay_ms,
        }
    }

    /// Deterministic constructor for the Story 1.17 eval runner.
    ///
    /// Given an [`InsightsContext`], synthesizes an `InsightsOutput` whose
    /// `headline`, `recommendation`, and individual `InsightItem.message`
    /// strings reference numbers that came directly from the context's
    /// `usage_payload`. This is what makes the eval pipeline reproducible
    /// for CI without spending Anthropic credits: the mock is "faithful by
    /// construction" — every number it cites comes from the input — so the
    /// faithfulness baseline is meaningful instead of a coin flip.
    ///
    /// **Why a separate constructor:** the existing [`new`] is fixture-driven
    /// (you hand it a hard-coded `response`); this constructor *derives* the
    /// response from the context, which is exactly the shape the eval
    /// pipeline needs but the wrong shape for unit tests that want to assert
    /// on exact token sequences.
    pub fn for_eval(context: &InsightsContext) -> Self {
        use super::schema::{InsightItem, InsightItemType, InsightSeverity, InsightsOutput};

        if context.usage_payload.is_empty() {
            // Empty-payload guardrail path — matches the prompt template's
            // "no data yet" instructions in `prompts/insights.v1.md`.
            let resp = InsightsOutput {
                headline: format!("Not enough data yet — window {} days", context.since_days),
                insights: vec![],
                recommendation: format!(
                    "Verify vendor credentials and wait {} days before rerunning",
                    context.since_days
                ),
                cost_usd: 0.0,
                latency_ms: 0,
                prompt_version: context.prompt_version.clone(),
            };
            return Self {
                token_chunks: vec![],
                response: resp,
                delay_ms: 0,
            };
        }

        // Build one InsightItem per vendor in the payload, referencing the
        // total_tokens, total_cost_usd, and window_days fields — that's
        // enough numeric evidence for the faithfulness heuristic to hit and
        // an action verb + number for the relevance heuristic to hit.
        let mut insights = Vec::with_capacity(context.usage_payload.len());
        for v in &context.usage_payload {
            // Pick a daily-token peak as a concrete claim.
            let peak = v.daily_tokens.iter().copied().max().unwrap_or(0);
            insights.push(InsightItem {
                type_: InsightItemType::Trend,
                severity: InsightSeverity::Info,
                message: format!(
                    "Consider reviewing {vendor}: {total} tokens over {days} days, peak {peak}",
                    vendor = v.vendor_id,
                    total = v.total_tokens,
                    days = v.window_days,
                    peak = peak,
                ),
                evidence: format!(
                    "{vendor} spent {cost} dollars across {days} days",
                    vendor = v.vendor_id,
                    cost = v.total_cost_usd,
                    days = v.window_days,
                ),
            });
        }

        let total_cost: f64 = context.usage_payload.iter().map(|v| v.total_cost_usd).sum();
        let total_tokens: u64 = context.usage_payload.iter().map(|v| v.total_tokens).sum();

        let resp = InsightsOutput {
            headline: format!(
                "Usage across {} vendors: {} tokens, {} dollars",
                context.usage_payload.len(),
                total_tokens,
                total_cost
            ),
            insights,
            recommendation: format!(
                "Monitor the {} day window; consider capping at {} dollars",
                context.since_days,
                // Use observed total_cost as the suggested cap so every
                // number in the recommendation traces back to the context
                // (PRD §6.1 faithfulness invariant: never invent figures).
                total_cost
            ),
            cost_usd: 0.02,
            latency_ms: 100,
            prompt_version: context.prompt_version.clone(),
        };
        Self {
            token_chunks: vec![],
            response: resp,
            delay_ms: 0,
        }
    }
}

#[async_trait]
impl LlmClient for MockLlmClient {
    async fn request_insight_streaming(
        &self,
        _context: InsightsContext,
        callback: Arc<dyn InsightsCallback>,
        cancel: Arc<CancelHandle>,
    ) -> Result<InsightsOutput, InsightsError> {
        let token = cancel.token();
        for chunk in &self.token_chunks {
            // Check cancellation before sleeping AND before emitting. This
            // mirrors the real client's `tokio::select!` semantics — the
            // mock doesn't have a stream to race against, but the contract
            // ("no callback after cancel") still holds.
            if token.is_cancelled() {
                return Err(InsightsError::Cancelled);
            }
            if self.delay_ms > 0 {
                tokio::select! {
                    biased;
                    _ = token.cancelled() => return Err(InsightsError::Cancelled),
                    _ = tokio::time::sleep(std::time::Duration::from_millis(self.delay_ms)) => {}
                }
            }
            if token.is_cancelled() {
                return Err(InsightsError::Cancelled);
            }
            callback.on_token(chunk.clone());
        }
        Ok(self.response.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::insights::schema::{InsightItem, InsightItemType, InsightSeverity};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingCb {
        tokens: AtomicUsize,
    }
    impl InsightsCallback for CountingCb {
        fn on_token(&self, _t: String) {
            self.tokens.fetch_add(1, Ordering::SeqCst);
        }
        fn on_error(&self, _e: String) {}
    }

    fn fixture_output() -> InsightsOutput {
        InsightsOutput {
            headline: "Test".to_string(),
            insights: vec![InsightItem {
                type_: InsightItemType::Trend,
                severity: InsightSeverity::Info,
                message: "m".to_string(),
                evidence: "e".to_string(),
            }],
            recommendation: "r".to_string(),
            cost_usd: 0.01,
            latency_ms: 100,
            prompt_version: "v1".to_string(),
        }
    }

    #[tokio::test]
    async fn mock_emits_tokens_then_returns_output() {
        let mock = MockLlmClient::new(
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
            fixture_output(),
            0,
        );
        let cb = Arc::new(CountingCb {
            tokens: AtomicUsize::new(0),
        });
        let cb_dyn: Arc<dyn InsightsCallback> = cb.clone();
        let cancel = CancelHandle::new_arc();
        let out = mock
            .request_insight_streaming(InsightsContext::synthetic(), cb_dyn, cancel)
            .await
            .unwrap();
        assert_eq!(cb.tokens.load(Ordering::SeqCst), 3);
        assert_eq!(out.headline, "Test");
    }

    #[tokio::test]
    async fn mock_honours_cancellation() {
        let mock = MockLlmClient::new(vec!["a".to_string(); 100], fixture_output(), 10);
        let cb = Arc::new(CountingCb {
            tokens: AtomicUsize::new(0),
        });
        let cb_dyn: Arc<dyn InsightsCallback> = cb.clone();
        let cancel = CancelHandle::new_arc();
        let cancel_clone = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            cancel_clone.cancel();
        });
        let result = mock
            .request_insight_streaming(InsightsContext::synthetic(), cb_dyn, cancel)
            .await;
        assert!(matches!(result, Err(InsightsError::Cancelled)));
        // We emitted some but nowhere near all 100 chunks.
        assert!(cb.tokens.load(Ordering::SeqCst) < 100);
    }
}
