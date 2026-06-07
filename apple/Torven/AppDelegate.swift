//
//  AppDelegate.swift
//  Torven
//
//  Story 5.2.1 iteration 4 (WAVE5-D8-impl-v4 FINAL — Pragmática 2):
//  AppDelegate minimal — `applicationShouldTerminateAfterLastWindowClosed { false }`.
//  Defensive only nesta iteração (não load-bearing): com `LSUIElement: true`
//  preservado iter 4, o app já é menu-bar accessory e não encerra ao fechar
//  janelas; mas manter o hook custa zero e protege defesa-em-profundidade
//  contra futura mudança de lifecycle (ex: Story 1.5+ App Sandbox que possa
//  alterar comportamento default).
//
//  Histórico das 3 iterações abandonadas (todas resetadas):
//    - Iter 1 (commit `8c620ee` resetado): `.onAppear`/`.onDisappear` em
//      SettingsView faziam flip dinâmico de `NSApp.activationPolicy` entre
//      `.accessory` e `.regular`. Smoke do Lorenzo (2026-06-06) mostrou app
//      encerrando ao fechar Settings — sem AppDelegate, NSApp default =
//      terminar quando última window `.regular` fecha.
//    - Iter 2 (commit `1992ed1` resetado): AppDelegate completo (~190 LOC)
//      com `applicationShouldTerminateAfterLastWindowClosed { false }` +
//      observers de `NSWindow.didBecomeKeyNotification` (promove `.regular`)
//      e `NSWindow.willCloseNotification` (reverte `.accessory`), filtrados
//      por `window.title == "Torven Settings"`. Build verde, mas smoke do
//      Lorenzo mostrou `NSViewBridgeErrorCanceled` persistente — `didBecomeKey`
//      é post-event, então o ViewBridge cancel já tinha ocorrido antes do
//      flip `.accessory → .regular` ser aplicado. Race condition fundamental.
//    - Iter 3 (commit `dbbd0f1` resetado, Pragmática 1): dropar `LSUIElement`
//      pra app rodar `.regular` desde launch + AppDelegate keep-alive. Smoke
//      Lorenzo: AC-1/2/4/6 PASS mas AC-3 FAIL — app ainda encerrava ao fechar
//      Settings mesmo com `applicationShouldTerminateAfterLastWindowClosed`
//      retornando `false`. Provável causa: quirk `MenuBarExtra` + `Settings { }`
//      lifecycle em macOS 13+.
//
//  Decisão Pragmática 2 (WAVE5-D8-impl-v4 FINAL, Lorenzo + Orion, 2026-06-06):
//  bypassa a `Settings { }` SwiftUI scene inteiramente. Settings UI agora
//  vive em `SettingsWindowController` (NSWindowController custom + NSHostingView).
//  `LSUIElement: true` preservado iter 4. AppDelegate retido como defensive
//  layer + ponto de extensão pra Story 1.15 (ADR-10) que vai adicionar
//  Keychain-shutdown wiring em `applicationWillTerminate(_:)`.
//

import AppKit
import Foundation

final class AppDelegate: NSObject, NSApplicationDelegate {
    /// Defensive: impede quit acidental se algum lifecycle path future
    /// remover `LSUIElement` ou introduzir window com policy `.regular`.
    /// Não load-bearing em iter 4 — `LSUIElement: true` já garante que
    /// o app seja menu-bar accessory e não encerre ao fechar windows.
    func applicationShouldTerminateAfterLastWindowClosed(
        _ sender: NSApplication
    ) -> Bool {
        return false
    }
}
