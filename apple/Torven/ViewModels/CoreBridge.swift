//
//  CoreBridge.swift
//  Torven
//
//  Swift-side bridge over the Rust core exposed by TorvenCore.xcframework.
//  Established in Story 1.5 as the canonical adapter from the FFI surface to
//  SwiftUI's reactive `ObservableObject` model.
//
//  Architecture: docs/architecture/torven-v1-adr.md#adr-10
//
//  ## Design notes
//
//  - `@MainActor` so every `@Published` mutation lands on the main thread
//    automatically. SwiftUI's diffing requires main-thread updates.
//  - Constructor is **synchronous**: Story 1.5 only calls `getVendorList()`,
//    which is a pure / fast Rust function. Async wrappers (for streaming
//    snapshots, AI insights, etc.) are added in Story 1.13+ using
//    UniFFI's `[Async]` annotation.
//  - The class is intentionally lean for Story 1.5. Future stories layer on:
//      * Story 1.15 — `static var shared` singleton + AppDelegate shutdown
//      * Story 1.13 — async snapshot polling
//      * Story 1.15 — AI Insights streaming via `AsyncSequence`
//      * Story 3.3 (Wave 3) — per-vendor accounts cache + active-account swap
//

import Foundation
import SwiftUI

@MainActor
final class TorvenCoreBridge: ObservableObject {
    /// LLM vendors the Rust core knows about. Populated once at init from
    /// `getVendorList()`. Story 1.6 will introduce a refresh path that
    /// re-probes config when the user edits credentials in the Settings
    /// overlay.
    @Published var vendors: [VendorInfo]

    /// Story 3.3 (Wave 3): per-vendor account cache populated lazily by
    /// `accounts(for:)`. Keyed by `vendor.id` slug ("openrouter", "zai").
    /// Vendors without multi-account return an empty array (graceful, not an
    /// error — see `get_accounts_for_vendor` in the Rust FFI).
    @Published var accountsByVendor: [String: [AccountInfo]] = [:]

    init() {
        // `getVendorList()` is a UniFFI-generated free function that
        // synchronously calls into the Rust staticlib bundled in
        // TorvenCore.xcframework. Story 1.5 hardcodes the result to 5
        // vendors with `isConfigured == false`.
        self.vendors = getVendorList()
    }

    /// Story 3.3: returns the configured accounts for `vendorId`, with the
    /// active marker reflecting the in-process Rust state. First call per
    /// vendor performs the FFI fetch and caches the result; subsequent
    /// callers within the same session hit the cache. After
    /// `swapActiveAccount(...)` succeeds the cache is mutated in place to
    /// reflect the new active row.
    func accounts(for vendorId: String) -> [AccountInfo] {
        if let cached = accountsByVendor[vendorId] {
            return cached
        }
        let fetched = getAccountsForVendor(vendorId: vendorId)
        accountsByVendor[vendorId] = fetched
        return fetched
    }

    /// Story 3.3: persists the active-account swap to Rust (in-memory for
    /// Wave 3, see `set_active_account` impl) and updates the local cache so
    /// the VendorCard re-renders with the new checkmark. On FFI error the
    /// cache is left untouched and the error is logged — graceful
    /// degradation per AC-4.
    func swapActiveAccount(vendorId: String, accountId: String) {
        do {
            try setActiveAccount(vendorId: vendorId, accountId: accountId)
        } catch {
            // Graceful degradation — log and bail out. UX: sheet has already
            // dismissed by the caller, so the card simply continues showing
            // the previous active marker.
            print("[TorvenCoreBridge] swapActiveAccount failed: \(error)")
            return
        }
        guard var rows = accountsByVendor[vendorId] else { return }
        for index in rows.indices {
            rows[index] = AccountInfo(
                id: rows[index].id,
                label: rows[index].label,
                isActive: rows[index].id == accountId
            )
        }
        accountsByVendor[vendorId] = rows
    }
}
