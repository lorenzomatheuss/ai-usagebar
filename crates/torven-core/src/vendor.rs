//! Shared vendor IDs and fetch-outcome wrapper used across every renderer
//! (SwiftUI bridge, developer TUI, AI Insights pipeline).
//!
//! Snapshots remain a discriminated `VendorSnapshot` enum because the four
//! vendors have genuinely different shapes — see `usage.rs`.
//!
//! ## Story 1.4 changes
//!
//! The `RenderOpts` struct and its `from_cli` impl lived here until Story 1.4
//! — they were a parameter-bag for the Waybar widget shell that no longer
//! exists. Renderers now consume [`crate::format::RawMetrics`] directly, so
//! per-vendor render-time options (icons, format strings, tolerance) are
//! either inlined where needed or moved to the surface that owns them (see
//! `format_tui.rs` in the `torven-tui` crate).

use std::time::Duration;

use crate::usage::VendorSnapshot;

/// Outer reqwest client timeout shared by every fetch entry point. Vendor
/// fetchers still apply their own tighter per-request timeouts.
pub const HTTP_CLIENT_TIMEOUT: Duration = Duration::from_secs(30);

/// Stable enum used by config files and across the FFI boundary.
#[derive(
    Debug, Clone, Copy, clap::ValueEnum, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize,
)]
#[serde(rename_all = "lowercase")]
pub enum VendorId {
    Anthropic,
    Openai,
    Zai,
    Openrouter,
}

impl VendorId {
    pub fn slug(self) -> &'static str {
        match self {
            VendorId::Anthropic => "anthropic",
            VendorId::Openai => "openai",
            VendorId::Zai => "zai",
            VendorId::Openrouter => "openrouter",
        }
    }

    pub fn all() -> &'static [VendorId] {
        &[
            VendorId::Anthropic,
            VendorId::Openai,
            VendorId::Zai,
            VendorId::Openrouter,
        ]
    }
}

/// What a vendor returns from a successful fetch — snapshot + meta. Mirrors
/// `anthropic::fetch::FetchOutcome` but vendor-agnostic.
#[derive(Debug, Clone)]
pub struct VendorOutcome {
    pub snapshot: VendorSnapshot,
    pub stale: bool,
    pub last_error: Option<(u16, String)>,
    pub cache_age: Option<std::time::Duration>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vendor_id_slug_round_trip() {
        for id in VendorId::all() {
            assert_eq!(
                id.slug(),
                serde_json::to_value(id).unwrap().as_str().unwrap()
            );
        }
    }
}
