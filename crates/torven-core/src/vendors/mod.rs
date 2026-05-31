//! Per-vendor fetchers (Anthropic, OpenAI, OpenRouter, Z.AI).
//!
//! Each submodule owns the HTTP shape, OAuth handling, and snapshot type for
//! one upstream API. They share the `cache::Cache` + `error::AppError` glue
//! but otherwise stay independent so a vendor outage or schema change can be
//! contained to its own module.
//!
//! ## Story 1.4 migration
//!
//! Moved from the legacy flat `src/{anthropic,openai,openrouter,zai}/` layout
//! into `crates/torven-core/src/vendors/{vendor}/`. The Waybar-coupled
//! per-vendor `vendor.rs` renderers were dropped — rendering is now the job
//! of the SwiftUI app (via UniFFI) or `torven-tui` (via `format_tui.rs`).
//!
//! `From<vendor::fetch::FetchOutcome> for VendorOutcome` impls live here so
//! consumers can call `outcome.into()` on any vendor's fetch result.

pub mod anthropic;
pub mod openai;
pub mod openrouter;
pub mod zai;

use crate::usage::VendorSnapshot;
use crate::vendor::VendorOutcome;

impl From<anthropic::fetch::FetchOutcome> for VendorOutcome {
    fn from(o: anthropic::fetch::FetchOutcome) -> Self {
        Self {
            snapshot: VendorSnapshot::Anthropic(o.snapshot),
            stale: o.stale,
            last_error: o.last_error,
            cache_age: o.cache_age,
        }
    }
}

impl From<openai::fetch::FetchOutcome> for VendorOutcome {
    fn from(o: openai::fetch::FetchOutcome) -> Self {
        Self {
            snapshot: VendorSnapshot::Openai(o.snapshot),
            stale: o.stale,
            last_error: o.last_error,
            cache_age: o.cache_age,
        }
    }
}

impl From<openrouter::fetch::FetchOutcome> for VendorOutcome {
    fn from(o: openrouter::fetch::FetchOutcome) -> Self {
        Self {
            snapshot: VendorSnapshot::Openrouter(o.snapshot),
            stale: o.stale,
            last_error: o.last_error,
            cache_age: o.cache_age,
        }
    }
}

impl From<zai::fetch::FetchOutcome> for VendorOutcome {
    fn from(o: zai::fetch::FetchOutcome) -> Self {
        Self {
            snapshot: VendorSnapshot::Zai(o.snapshot),
            stale: o.stale,
            last_error: o.last_error,
            cache_age: o.cache_age,
        }
    }
}
