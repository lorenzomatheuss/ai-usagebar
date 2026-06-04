//! AI Insights backend — Story 1.15.
//!
//! Public surface for the AI Insights subsystem: schema types, `LlmClient`
//! trait + implementations (`MockLlmClient` for tests, `RealAnthropicClient`
//! for production), budget guards, and the `CancelHandle` used by the FFI
//! cancellation strategy (AR-2 resolution §Decision 3).
//!
//! ## Module layout
//!
//! - [`schema`] — `InsightsContext`, `InsightsOutput`, `InsightsError`, JSON
//!   schema validator.
//! - [`llm_client`] — `LlmClient` trait + `MockLlmClient`.
//! - [`client`] — `RealAnthropicClient` (HTTP streaming against Anthropic
//!   Messages API in tool_use mode).
//! - [`budget`] — input-token estimation, cost-budget check, 1/min rate
//!   limiter.
//! - [`cancel`] — `CancelHandle` wrapping `tokio_util::sync::CancellationToken`
//!   (FFI-exposed surface for AR-2 cancellation).
//! - [`eval`] — stub for Story 1.17 (eval runner / judge pipeline).
//!
//! ## Architecture references
//!
//! - `docs/architecture/ar-2-spike-resolution.md` — async FFI shape +
//!   cancellation strategy (BINDING).
//! - `docs/architecture/torven-v1-adr.md` §ADR-7 — Anthropic tool_use mode,
//!   tokio runtime ownership.
//! - `docs/prd/torven-v1.md` §6 — AI Insights product spec, privacy
//!   redaction rules.

pub mod budget;
pub mod cancel;
pub mod client;
pub mod eval;
pub mod llm_client;
pub mod schema;

pub use cancel::CancelHandle;
pub use client::RealAnthropicClient;
pub use llm_client::{InsightsCallback, LlmClient, MockLlmClient};
pub use schema::{
    InsightItem, InsightItemType, InsightSeverity, InsightsContext, InsightsError, InsightsOutput,
    VendorAggregate, validate_output,
};
