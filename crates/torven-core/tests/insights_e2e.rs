//! Integration tests for the AI Insights backend (Story 1.15 AC-9).
//!
//! All tests here use `MockLlmClient` — no network. The real Anthropic
//! client is exercised by the AR-2 spike (`tests/ar2_async_ffi_spike.rs`)
//! against an in-process mockito-style SSE server, and by the (manual,
//! out-of-CI) `tests/live.rs` smoke against the actual Anthropic
//! endpoint.

#![cfg(test)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use torven_core::insights::budget::{MAX_INPUT_TOKENS, check_cost_budget, estimate_input_tokens};
use torven_core::insights::{
    CancelHandle, InsightItem, InsightItemType, InsightSeverity, InsightsCallback, InsightsContext,
    InsightsError, InsightsOutput, LlmClient, MockLlmClient, VendorAggregate,
};

struct CountingCallback {
    tokens: AtomicUsize,
}

impl InsightsCallback for CountingCallback {
    fn on_token(&self, _token: String) {
        self.tokens.fetch_add(1, Ordering::SeqCst);
    }
    fn on_error(&self, _error: String) {}
}

fn fixture_output() -> InsightsOutput {
    InsightsOutput {
        headline: "Usage trending up".to_string(),
        insights: vec![InsightItem {
            type_: InsightItemType::Trend,
            severity: InsightSeverity::Info,
            message: "Token usage increased".to_string(),
            evidence: "Daily tokens grew from 10k to 25k".to_string(),
        }],
        recommendation: "Monitor for the next 7 days.".to_string(),
        cost_usd: 0.012,
        latency_ms: 1500,
        prompt_version: "v1".to_string(),
    }
}

fn small_ctx() -> InsightsContext {
    InsightsContext::new(vec![VendorAggregate {
        vendor_id: "anthropic".to_string(),
        account_id: Some("hash1234".to_string()),
        total_tokens: 120_000,
        total_cost_usd: 4.20,
        window_days: 7,
        daily_tokens: vec![10_000, 15_000, 20_000, 25_000, 20_000, 15_000, 15_000],
        daily_cost_usd: vec![0.35, 0.52, 0.70, 0.87, 0.70, 0.52, 0.54],
        tag: None,
    }])
}

#[tokio::test]
async fn test_insights_with_mock() {
    // Three deterministic chunks; mock concatenates them and returns the
    // configured InsightsOutput as the future value (per AR-2 §Decision 1).
    let chunks = vec![
        "{\"headline\":\"".to_string(),
        "Usage trending up\"".to_string(),
        ", ...rest...}".to_string(),
    ];
    let client = MockLlmClient::new(chunks, fixture_output(), 0);
    let cb = Arc::new(CountingCallback {
        tokens: AtomicUsize::new(0),
    });
    let cb_dyn: Arc<dyn InsightsCallback> = cb.clone();
    let cancel = CancelHandle::new_arc();

    let output = client
        .request_insight_streaming(small_ctx(), cb_dyn, cancel)
        .await
        .expect("mock should produce a valid InsightsOutput");

    assert_eq!(
        cb.tokens.load(Ordering::SeqCst),
        3,
        "all three chunks emitted"
    );
    assert_eq!(output.headline, "Usage trending up");
    assert_eq!(output.prompt_version, "v1");
    assert_eq!(output.insights.len(), 1);
}

#[tokio::test]
async fn test_budget_exceeded() {
    // Build a context whose JSON serialization clearly exceeds 32k chars
    // (≈ 8000 tokens at chars/4). The budget guard rejects it without
    // ever invoking the mock.
    let huge_ctx = InsightsContext::new(
        (0..500)
            .map(|i| VendorAggregate {
                vendor_id: format!("vendor{i}"),
                account_id: Some("hash1234".to_string()),
                total_tokens: 1_000_000,
                total_cost_usd: 99.99,
                window_days: 365,
                daily_tokens: vec![1_000_000; 365],
                daily_cost_usd: vec![99.99; 365],
                tag: Some("a-long-and-verbose-tag-value-for-padding".to_string()),
            })
            .collect(),
    );

    let tokens = estimate_input_tokens("template", &huge_ctx);
    assert!(
        tokens > MAX_INPUT_TOKENS,
        "huge_ctx must overshoot the budget (got {tokens} tokens, limit {MAX_INPUT_TOKENS})"
    );
    let err = check_cost_budget(tokens, "claude-3-5-sonnet-20241022").unwrap_err();
    match err {
        InsightsError::InputBudgetExceeded { tokens: t } => assert_eq!(t, tokens),
        other => panic!("expected InputBudgetExceeded, got {other:?}"),
    }
}

#[tokio::test]
async fn test_privacy_no_api_keys() {
    // Round-trip the privacy-redacted context through serde_json and assert
    // no `api_key`, `access_token`, `refresh_token`, or plaintext account
    // names appear in the serialized payload that would cross to Anthropic.
    let ctx = small_ctx();
    let json = serde_json::to_string(&ctx).expect("ctx must serialize");
    for forbidden in [
        "api_key",
        "apiKey",
        "access_token",
        "refresh_token",
        "Bearer ",
        "sk-",
        "Acme",
    ] {
        assert!(
            !json.contains(forbidden),
            "InsightsContext JSON contains forbidden substring `{forbidden}`: {json}"
        );
    }
    // Account ID is the hashed short form (or absent), not a real account name.
    for agg in &ctx.usage_payload {
        if let Some(id) = &agg.account_id {
            assert!(
                id.len() <= 16,
                "account_id `{id}` longer than 16 chars suggests plaintext leakage"
            );
        }
    }
}
