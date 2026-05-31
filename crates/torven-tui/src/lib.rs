//! Developer-facing TUI for Torven — one tab per enabled vendor with live
//! refresh and an interactive settings overlay.
//!
//! Story 1.4 moved the legacy `src/tui/*` modules into this dedicated crate
//! and introduced [`format_tui`] as the bridge between the new
//! [`torven_core::format::RawMetrics`] surface and ratatui's
//! [`ratatui::style::Color`] primitives. The Linux-specific Waybar plumbing
//! (`waybar::request_refresh`, Pango markup) is gone — saves are durable on
//! disk and propagate on the next refresh tick.

pub mod app;
pub mod format_tui;
pub mod panels;
pub mod settings;
pub mod view;
