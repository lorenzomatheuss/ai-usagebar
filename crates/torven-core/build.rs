//! Build script — generates UniFFI scaffolding from `src/torven_core.udl`.
//!
//! Runs at compile time before the crate body is parsed. Emits a Rust source
//! file (under `OUT_DIR`) that `src/uniffi_exports.rs` then includes via
//! `uniffi::include_scaffolding!`. Without this step the `#[uniffi::export]`
//! annotations have no glue to bind to.
//!
//! See ADR-4 (UniFFI binding tool) for the rationale.

fn main() {
    uniffi::generate_scaffolding("src/torven_core.udl").unwrap();
}
