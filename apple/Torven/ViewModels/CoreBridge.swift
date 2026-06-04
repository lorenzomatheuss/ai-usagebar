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

    init() {
        // `getVendorList()` is a UniFFI-generated free function that
        // synchronously calls into the Rust staticlib bundled in
        // TorvenCore.xcframework. Story 1.5 hardcodes the result to 5
        // vendors with `isConfigured == false`.
        self.vendors = getVendorList()
    }
}
