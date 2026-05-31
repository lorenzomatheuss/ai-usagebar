//! torven library — shared core for the developer TUI binary.
//!
//! Legacy flat layout — these modules will be migrated into
//! `crates/torven-core/` in Story 1.4. After the migration this file
//! disappears entirely. Until then it documents what survived the
//! Waybar/Linux cleanup (Story 1.3): vendor fetchers + cross-platform
//! primitives only.
//!
//! Modules deleted in Story 1.3 (Waybar/Pango/Omarchy coupling):
//! - `active`, `pango`, `theme`, `tooltip`, `waybar`, `widget`
//!
//! NOTE: this crate has no `[package]` entry in root `Cargo.toml`; it is
//! kept on disk purely as the source of truth for the Story 1.4 migration.

pub mod anthropic;
pub mod cache;
pub mod config;
pub mod countdown;
pub mod error;
pub mod format;
pub mod openai;
pub mod openrouter;
pub mod pacing;
pub mod tui;
pub mod usage;
pub mod vendor;
pub mod zai;

// TODO Story 1.2/1.4: adicionar pub mod uniffi_exports; pub mod insights; pub mod history; pub mod keychain;

pub use error::{AppError, Result};
