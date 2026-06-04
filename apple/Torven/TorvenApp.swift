//
//  TorvenApp.swift
//  Torven
//
//  Menu-bar app entry point. Story 1.1 bootstrapped a "hello world" Text;
//  Story 1.5 wired the Rust → SwiftUI bridge end-to-end. Story 2.1 (Wave 2)
//  decoupled the content view via the `MenuBarContent` wrapper. Story 2.2
//  extracts the SF Symbol into `MenuBarLabel` (configurable skeleton) so
//  Wave 7 (AR-8) can swap to a dynamic label without touching this entry
//  point.
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
        MenuBarExtra {
            MenuBarContent()
                .environmentObject(coreBridge)
        } label: {
            MenuBarLabel(symbol: MenuBarLabel.defaultSymbol)
        }
        .menuBarExtraStyle(.window)
    }
}
