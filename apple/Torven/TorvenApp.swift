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
//  Story 5.2.1 iteration 4 (WAVE5-D8-impl-v4 FINAL — Pragmática 2): a
//  `Settings { SettingsView() }` SwiftUI scene foi REMOVIDA. 3 iterações
//  (1: .onAppear flip, 2: AppDelegate + observers, 3: drop LSUIElement)
//  falharam em torno da `Settings { }` scene — sinal de design dead-end.
//  Saída: bypassa a scene. Settings UI agora vive em
//  `SettingsWindowController` (NSWindowController custom + NSHostingView
//  wrapping `SettingsView()`), mesmo pattern da `MainWindowController`/
//  Story 4.1. `⌘,` wired via `KeyboardShortcuts` package (mesmo pattern
//  do `⌘1`). `LSUIElement: true` PRESERVADO — app segue menu-bar-only
//  sem dock icon, como originalmente planejado.
//
//  See ADR-10 for the broader app skeleton. `AppDelegate` retido como
//  defensive (`applicationShouldTerminateAfterLastWindowClosed { false }`)
//  e ponto de extensão pra Story 1.15 (Keychain-shutdown wiring em
//  `applicationWillTerminate`).
//

import KeyboardShortcuts
import SwiftUI

extension KeyboardShortcuts.Name {
    /// Global hotkey to bring the Main Window to focus from anywhere on
    /// the system. Default: ⌘1 (see TorvenApp.init for the default-once
    /// guard so the user's recorded shortcut isn't overwritten on relaunch).
    static let openMainWindow = Self("openMainWindow")

    /// Story 5.2.1 iter 4 (WAVE5-D8-impl-v4): substitui o `⌘,` auto-gerado
    /// pelo macOS via SwiftUI `Settings { }` scene (que foi removida — ver
    /// `SettingsWindowController.swift` pra histórico). Default: ⌘,
    /// (KeyboardShortcuts.setShortcut em `configureGlobalHotkeys` aplica
    /// na primeira execução via `defaultShortcutAppliedKey` guard).
    static let openSettings = Self("openSettings")
}

@main
struct TorvenApp: App {
    // Story 5.2.1 iter 4: `AppDelegate` retido como defensive layer e
    // ponto de extensão pra Story 1.15 (ADR-10, Keychain-shutdown wiring).
    // Com `LSUIElement: true` preservado iter 4, o app já é menu-bar-only
    // sem precisar de keep-alive — mas manter custa zero (sem observers,
    // sem closures, sem retain cycles) e protege defesa-em-profundidade
    // contra futura mudança de lifecycle.
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate

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

        // Story 5.2.1 iter 4: a SwiftUI `Settings { SettingsView() }` scene
        // foi removida daqui. 3 iterações falharam em torno dela — bypassa
        // inteiramente. Settings UI agora é wrapped via NSHostingView dentro
        // de NSWindow programático (ver `SettingsWindowController.swift`),
        // wired via KeyboardShortcuts `⌘,` em `configureGlobalHotkeys()`
        // abaixo. WAVE5-D2 ajustada: Settings continua "não NSPanel" —
        // agora é "NSWindow programmatic wrapping SwiftUI via NSHostingView",
        // que é stronger (mais controle, zero XPC).
    }

    // MARK: - Hotkey wiring

    private static let mainWindowDefaultShortcutAppliedKey = "TorvenMainWindowDefaultShortcutApplied"
    private static let settingsDefaultShortcutAppliedKey = "TorvenSettingsDefaultShortcutApplied"

    private static func configureGlobalHotkeys() {
        // Story 4.1: ⌘1 → MainWindow. Apply the default only on first launch
        // — once the user opens the (future) settings recorder, their choice
        // is what KeyboardShortcuts persists in UserDefaults; re-applying
        // the default each launch would silently overwrite that.
        if !UserDefaults.standard.bool(forKey: mainWindowDefaultShortcutAppliedKey) {
            KeyboardShortcuts.setShortcut(
                .init(.one, modifiers: .command),
                for: .openMainWindow
            )
            UserDefaults.standard.set(true, forKey: mainWindowDefaultShortcutAppliedKey)
        }

        KeyboardShortcuts.onKeyUp(for: .openMainWindow) {
            MainWindowController.shared.show()
        }

        // Story 5.2.1 iter 4: ⌘, → Settings. Mesmo pattern do ⌘1: default
        // aplicado uma única vez via `settingsDefaultShortcutAppliedKey`
        // guard, depois respeita escolha do usuário em UserDefaults.
        // Substitui o `⌘,` auto-gerado pelo macOS via SwiftUI Settings
        // scene (removida iter 4 — ver `SettingsWindowController.swift`
        // pra histórico das 3 iterações).
        if !UserDefaults.standard.bool(forKey: settingsDefaultShortcutAppliedKey) {
            KeyboardShortcuts.setShortcut(
                .init(.comma, modifiers: .command),
                for: .openSettings
            )
            UserDefaults.standard.set(true, forKey: settingsDefaultShortcutAppliedKey)
        }

        KeyboardShortcuts.onKeyUp(for: .openSettings) {
            SettingsWindowController.shared.show()
        }
    }
}
