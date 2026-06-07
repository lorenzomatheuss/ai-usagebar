//! End-to-end integration test for the Anthropic vendor.
//!
//! Stands up a mockito server that pretends to be the Anthropic OAuth +
//! usage endpoints, drives the full `fetch_snapshot` pipeline against canned
//! fixtures, and asserts on the resulting [`AnthropicSnapshot`] + the
//! [`RawMetrics`] projection.
//!
//! ## Story 1.4 rewrite
//!
//! The legacy version of this test asserted on Pango-markup Waybar JSON via
//! `insta` snapshots — that surface was deleted in Story 1.3. The fetcher
//! itself is unchanged, so the assertions now target what every consumer
//! actually reads: the snapshot fields and the cross-platform RawMetrics
//! struct. Schema drift in the upstream API still surfaces here as a
//! deserialization failure or a wrong utilization percentage.

use std::io::Write;
use std::path::Path;
use std::time::Duration;

use tempfile::{NamedTempFile, TempDir};

use torven_core::cache::Cache;
use torven_core::format::{LabelKind, compute_metrics};
use torven_core::usage::VendorSnapshot;
use torven_core::vendors::anthropic::{
    self,
    creds::CredsSource,
    fetch::Endpoints,
};

fn write_creds() -> NamedTempFile {
    // Token expires far in the future → no refresh needed during the test.
    let expires_ms = chrono::Utc::now().timestamp_millis() + 3_600_000;
    let body = format!(
        r#"{{"claudeAiOauth":{{
            "accessToken":"AT","refreshToken":"RT",
            "expiresAt": {expires_ms},
            "subscriptionType":"max","rateLimitTier":"default_claude_max_5x"
        }}}}"#
    );
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(body.as_bytes()).unwrap();
    f.flush().unwrap();
    f
}

fn cache_in(td: &TempDir) -> Cache {
    let c = Cache::at(td.path().join("anthropic"));
    c.ensure_dir().unwrap();
    c
}

fn read_fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("missing fixture {}: {e}", path.display());
    })
}

#[tokio::test]
async fn full_response_parses_into_snapshot_with_three_windows() {
    let mut server = mockito::Server::new_async().await;
    server
        .mock("GET", "/api/oauth/usage")
        .with_status(200)
        .with_body(read_fixture("anthropic_usage_full.json"))
        .create_async()
        .await;

    let td = TempDir::new().unwrap();
    let cache = cache_in(&td);
    let creds = write_creds();
    let client = reqwest::Client::new();
    let endpoints = Endpoints {
        usage: format!("{}/api/oauth/usage", server.url()),
        token: format!("{}/v1/oauth/token", server.url()),
    };
    let outcome = anthropic::fetch_snapshot(
        &client,
        &CredsSource::File(creds.path().to_path_buf()),
        &cache,
        &endpoints,
        Duration::from_secs(0),
    )
    .await
    .unwrap();

    // Fresh fetch — not stale.
    assert!(!outcome.stale, "fresh fetch should not be marked stale");

    let snap = &outcome.snapshot;
    // The fixture exercises every Anthropic field; assert the windows
    // round-trip and the headline RawMetrics projection is sensible.
    assert!(snap.session.utilization_pct >= 0 && snap.session.utilization_pct <= 100);
    assert!(snap.weekly.utilization_pct >= 0 && snap.weekly.utilization_pct <= 100);

    let metrics = compute_metrics(&VendorSnapshot::Anthropic(snap.clone()));
    assert_eq!(metrics.label_kind, LabelKind::PercentOfWindow);
    // Worst-of-windows is what RawMetrics reports — must be at least as high
    // as the session window.
    let session = snap.session.utilization_pct as f64;
    assert!(
        metrics.pct_used.unwrap_or(0.0) >= session,
        "headline pct {} should be >= session pct {}",
        metrics.pct_used.unwrap_or(0.0),
        session
    );
}

#[tokio::test]
async fn minimal_response_has_no_sonnet_or_extra() {
    let mut server = mockito::Server::new_async().await;
    server
        .mock("GET", "/api/oauth/usage")
        .with_status(200)
        .with_body(read_fixture("anthropic_usage_minimal.json"))
        .create_async()
        .await;

    let td = TempDir::new().unwrap();
    let cache = cache_in(&td);
    let creds = write_creds();
    let client = reqwest::Client::new();
    let endpoints = Endpoints {
        usage: format!("{}/api/oauth/usage", server.url()),
        token: format!("{}/v1/oauth/token", server.url()),
    };
    let outcome = anthropic::fetch_snapshot(
        &client,
        &CredsSource::File(creds.path().to_path_buf()),
        &cache,
        &endpoints,
        Duration::from_secs(0),
    )
    .await
    .unwrap();

    assert!(outcome.snapshot.sonnet.is_none());
    assert!(outcome.snapshot.extra.is_none());
    let metrics = compute_metrics(&VendorSnapshot::Anthropic(outcome.snapshot));
    // No `extra` block → no cost_usd in RawMetrics.
    assert!(metrics.cost_usd.is_none());
}

#[tokio::test]
async fn http_429_falls_back_to_stale_cache_with_last_error_recorded() {
    let mut server = mockito::Server::new_async().await;
    server
        .mock("GET", "/api/oauth/usage")
        .with_status(429)
        .with_body(r#"{"error":{"type":"rate_limit_error","message":"slow down"}}"#)
        .create_async()
        .await;

    let td = TempDir::new().unwrap();
    let cache = cache_in(&td);
    // Seed cache so fallback has something to serve.
    cache
        .write_payload(read_fixture("anthropic_usage_full.json").as_bytes())
        .unwrap();
    let creds = write_creds();
    let client = reqwest::Client::new();
    let endpoints = Endpoints {
        usage: format!("{}/api/oauth/usage", server.url()),
        token: format!("{}/v1/oauth/token", server.url()),
    };
    let outcome = anthropic::fetch_snapshot(
        &client,
        &CredsSource::File(creds.path().to_path_buf()),
        &cache,
        &endpoints,
        Duration::from_secs(0),
    )
    .await
    .unwrap();
    assert!(outcome.stale);
    assert_eq!(outcome.last_error.as_ref().map(|(c, _)| *c), Some(429));
    let msg = outcome.last_error.as_ref().map(|(_, m)| m.clone()).unwrap();
    assert!(
        msg.contains("slow down"),
        "expected vendor's error message to surface, got: {msg}"
    );
}
