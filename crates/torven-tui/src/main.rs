//! torven-tui — developer-facing TUI for diagnostics and debugging.
//!
//! Stub binary created by Story 1.1 (bootstrap-workspace). The real ratatui
//! panels currently in the legacy `src/tui/` will be migrated here in Story
//! 1.4.

fn main() {
    println!("torven-tui");
    // Sanity-check the workspace dependency wiring.
    println!("linked: {}", torven_core::ping());
}
