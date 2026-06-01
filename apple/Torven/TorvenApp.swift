//
//  TorvenApp.swift
//  Torven
//
//  Menu-bar app entry point. Story 1.1 bootstrapped a "hello world" Text;
//  Story 1.5 wires the real Rust → SwiftUI bridge: the app owns a
//  `TorvenCoreBridge` at the App scope and passes it to `VendorListView` via
//  `.environmentObject(...)`.
//
//  See ADR-10 for the broader app skeleton (Story 1.15 will add
//  `@NSApplicationDelegateAdaptor` + Keychain-shutdown wiring).
//

import SwiftUI

@main
struct TorvenApp: App {
    // `@StateObject` ensures the bridge is created once and survives Scene
    // re-evaluations. Story 1.5 only calls a synchronous Rust function in
    // its initializer, so creating it eagerly is cheap.
    @StateObject private var coreBridge = TorvenCoreBridge()

    var body: some Scene {
        MenuBarExtra("Torven", systemImage: "chart.bar.fill") {
            VendorListView()
                .environmentObject(coreBridge)
        }
        .menuBarExtraStyle(.window)
    }
}
