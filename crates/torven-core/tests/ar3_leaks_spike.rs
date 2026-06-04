//! AR-3 memory ownership spike — Story 1.5.
//!
//! This integration test calls `get_vendor_list()` in a tight loop to give
//! `leaks --atExit` enough volume to detect any allocations that escape the
//! FFI handoff. Run under leaks via:
//!
//!     leaks --atExit -- \
//!         cargo test --release -p torven-core --test ar3_leaks_spike \
//!             -- --nocapture --ignored
//!
//! The test is `#[ignore]` so it doesn't run as part of `cargo test`. The
//! Story 1.5 dev workflow ran it manually and pasted the `leaks` output
//! into the Change Log (zero malloc nodes leaked).
//!
//! ## Why this approach (vs. running leaks on the live SwiftUI app)
//!
//! Running `leaks` on the live `Torven.app` only exercises the *Swift→Rust*
//! direction once at launch (during `TorvenCoreBridge.init`). That confirms
//! the happy path doesn't leak but doesn't put repeated pressure on the
//! `Vec<VendorInfo>` lowering. This Rust-side spike loops the call 1,000
//! times so `leaks` has signal to flag any per-call accumulator.

use torven_core::get_vendor_list;

#[test]
#[ignore = "AR-3 spike — run under `leaks --atExit -- cargo test ... --ignored`"]
fn ar3_get_vendor_list_loop_leaks_check() {
    const ITERATIONS: usize = 1000;
    let mut total = 0usize;
    for _ in 0..ITERATIONS {
        let vendors = get_vendor_list();
        total += vendors.len();
        // Drop vendors here so any per-call allocation is freed before the
        // next iteration — leaks would otherwise see ITERATIONS * 5 live
        // VendorInfo records and report them as "in use" not "leaked".
        drop(vendors);
    }
    assert_eq!(total, ITERATIONS * 5);
    println!("AR-3 spike: {ITERATIONS} iterations completed, total = {total}");
}
