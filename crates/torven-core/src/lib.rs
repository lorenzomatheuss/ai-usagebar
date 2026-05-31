//! torven-core — Rust core library for Torven (macOS LLM observability).
//!
//! The real business modules (vendors, cache, config, AI insights, etc.) are
//! migrated from the legacy flat `src/` layout into this crate over Stories
//! 1.3 and 1.4. Story 1.2 wires up the UniFFI binding pipeline so that the
//! SwiftUI app in `apple/Torven/` can consume the Rust surface.
//!
//! The FFI surface is declared in `torven_core.udl` and implemented in
//! [`uniffi_exports`]. See `docs/architecture/torven-v1-adr.md#adr-4` for the
//! binding-tool rationale.
//!
//! ## UniFFI scaffolding layout
//!
//! `uniffi::include_scaffolding!` MUST live at the crate root because the
//! macro emits a `UniFfiTag` type that the generated `#[uniffi::export_for_udl]`
//! attributes resolve as `crate::UniFfiTag`. Implementations of UDL-declared
//! functions live in [`uniffi_exports`] and are re-exported here so the
//! generated scaffolding can find them via `crate::ping()`.

pub mod uniffi_exports;

pub use uniffi_exports::ping;

uniffi::include_scaffolding!("torven_core");
