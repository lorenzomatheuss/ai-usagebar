//
//  TorvenApp.swift
//  Torven
//
//  Menu-bar app entry point. Story 1.1 bootstrapped a "hello world" Text;
//  Story 1.5 wired the Rust → SwiftUI bridge end-to-end. Story 2.1 (Wave 2)
//  decoupled the content view via the `MenuBarContent` wrapper. Story 2.2
//  extracts the SF Symbol into `MenuBarLabel` (configurable skeleton) so
//  Wave 7 (AR-8) can swap to a dynamic label without touching this entry
//  point. Story 4.1 (Wave 4) registers the ⌘1 global hotkey that opens
//  the Main Window — second vector of WAVE4-D1's dual invocation pattern.
//
//  See ADR-10 for the broader app skeleton (Story 1.15 will add
//  `@NSApplicationDelegateAdaptor` + Keychain-shutdown wiring).
//

import KeyboardShortcuts
import SwiftUI

extension KeyboardShortcuts.Name {
    /// Global hotkey to bring the Main Window to focus from anywhere on
    /// the system. Default: ⌘1 (see TorvenApp.init for the default-once
    /// guard so the user's recorded shortcut isn't overwritten on relaunch).
    static let openMainWindow = Self("openMainWindow")
}

@main
struct TorvenApp: App {
    // `@StateObject` ensures the bridge is created once and survives Scene
    // re-evaluations. Story 1.5 only calls a synchronous Rust function in
    // its initializer, so creating it eagerly is cheap.
    @StateObject private var coreBridge = TorvenCoreBridge()

    init() {
        Self.configureGlobalHotkeys()
    }

    var body: some Scene {
        MenuBarExtra {
            MenuBarContent()
                .environmentObject(coreBridge)
        } label: {
            MenuBarLabel(symbol: MenuBarLabel.defaultSymbol)
        }
        .menuBarExtraStyle(.window)

        // Story 5.2 (Wave 5): native macOS Settings scene. The `Settings { }`
        // scene type wires ⌘, to "Settings…" automatically on macOS 13+. The
        // window is a singleton owned by SwiftUI; opening it twice focuses
        // the existing one rather than spawning a duplicate. Decision
        // WAVE5-D2 (cravada) — chosen over NSPanel for sandboxability and
        // simpler lifecycle. No coupling to `MenuBarExtra` observed in
        // testing on macOS 13.0 / 13.5 / 14.x.
        Settings {
            SettingsView()
        }
    }

    // MARK: - Hotkey wiring

    private static let defaultShortcutAppliedKey = "TorvenMainWindowDefaultShortcutApplied"

    private static func configureGlobalHotkeys() {
        // Apply the ⌘1 default only on first launch. Once the user opens the
        // (future) settings recorder, their choice is what KeyboardShortcuts
        // persists in UserDefaults — re-applying the default each launch
        // would silently overwrite that.
        if !UserDefaults.standard.bool(forKey: defaultShortcutAppliedKey) {
            KeyboardShortcuts.setShortcut(
                .init(.one, modifiers: .command),
                for: .openMainWindow
            )
            UserDefaults.standard.set(true, forKey: defaultShortcutAppliedKey)
        }

        KeyboardShortcuts.onKeyUp(for: .openMainWindow) {
            MainWindowController.shared.show()
        }
    }
}
