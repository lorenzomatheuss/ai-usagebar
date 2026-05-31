//! Thin wrapper around `uniffi::uniffi_bindgen_main()`.
//!
//! Following the recommended UniFFI 0.27+ pattern (see the UniFFI book,
//! "Tutorial / Recommended Approach"), the bindgen tool ships as a binary
//! inside the crate that depends on `uniffi`. This guarantees the bindgen
//! version is locked to the runtime version — historically the most common
//! cause of UniFFI codegen mismatches.
//!
//! Invoke as:
//!
//!     cargo run --features uniffi-cli \
//!       --bin uniffi-bindgen -- \
//!       generate crates/torven-core/src/torven_core.udl \
//!       --language swift --out-dir target/uniffi-swift
//!
//! See `apple/scripts/build-xcframework.sh` for the full XCFramework flow
//! that wraps this command (implemented in Story 1.5).

fn main() {
    uniffi::uniffi_bindgen_main()
}
