//
//  TorvenApp.swift
//  Torven
//
//  Menu-bar app entry point. Story 1.1 bootstrapped a "hello world" Text;
//  Story 1.5 wired the Rust → SwiftUI bridge end-to-end. Story 2.1 (Wave 2)
//  decoupled the content view via the `MenuBarContent` wrapper so future
//  stories (2.3 PopoverView, Wave 7 dynamic label) can swap implementations
//  without touching this entry point.
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
            MenuBarContent()
                .environmentObject(coreBridge)
        }
        .menuBarExtraStyle(.window)
    }
}
