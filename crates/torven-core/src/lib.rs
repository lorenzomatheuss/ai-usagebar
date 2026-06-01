//! torven-core — Rust core library for Torven (macOS LLM observability).
//!
//! ## Story 1.4 layout
//!
//! Story 1.4 migrated every cross-platform module out of the legacy flat
//! `src/` layout into this crate:
//!
//! - [`vendors`] — Anthropic / OpenAI / OpenRouter / Z.AI fetchers (HTTP +
//!   OAuth + types)
//! - [`cache`], [`countdown`], [`error`], [`pacing`], [`usage`], [`vendor`],
//!   [`config`] — cross-platform primitives
//! - [`format`] — [`RawMetrics`] projection (replaces the Pango-based
//!   `format.rs` that was deleted in Story 1.3 / refactored in 1.4)
//!
//! The SwiftUI app consumes the Rust surface through the FFI declared in
//! `torven_core.udl` (see [`uniffi_exports`]). The developer TUI lives in the
//! sibling `torven-tui` crate.
//!
//! ## UniFFI scaffolding layout
//!
//! `uniffi::include_scaffolding!` MUST live at the crate root because the
//! macro emits a `UniFfiTag` type that the generated `#[uniffi::export_for_udl]`
//! attributes resolve as `crate::UniFfiTag`. Implementations of UDL-declared
//! functions live in [`uniffi_exports`] and are re-exported here so the
//! generated scaffolding can find them via `crate::ping()`.

pub mod cache;
pub mod config;
pub mod countdown;
pub mod error;
pub mod format;
pub mod pacing;
pub mod uniffi_exports;
pub mod usage;
pub mod vendor;
pub mod vendors;

pub use error::{AppError, Result};
pub use format::{LabelKind, RawMetrics, compute_metrics};
pub use uniffi_exports::{VendorInfo, get_vendor_list, ping};

uniffi::include_scaffolding!("torven_core");
