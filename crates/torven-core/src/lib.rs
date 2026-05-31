//! torven-core — Rust core library for Torven (macOS LLM observability).
//!
//! This crate is a stub created by Story 1.1 (bootstrap-workspace). The real
//! core modules (vendors, cache, config, etc.) will be migrated from the
//! legacy flat `src/` layout in Stories 1.3 and 1.4.

/// Returns the crate's identity string. Used for smoke-testing the workspace
/// build pipeline before any real functionality is migrated.
pub fn ping() -> String {
    "torven-core".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_returns_crate_name() {
        assert_eq!(ping(), "torven-core");
    }
}
