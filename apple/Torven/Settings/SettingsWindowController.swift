//
//  SettingsWindowController.swift
//  Torven
//
//  Story 5.2.1 iteration 4 (WAVE5-D8-impl-v4 FINAL — Pragmática 2):
//  NSWindowController singleton que hospeda a Settings UI via NSHostingView
//  wrapping o SwiftUI `SettingsView()`. Substitui completamente a SwiftUI
//  `Settings { }` scene (que era a decisão WAVE5-D2 original mas provou-se
//  incompatível com o uso do Torven após 3 iterações falhas).
//
//  Histórico das 3 iterações abandonadas:
//    - Iter 1 (commit resetado): `.onAppear`/`.onDisappear` em SettingsView
//      tentava flipar `NSApp.activationPolicy` entre `.accessory` e `.regular`.
//      Sem `applicationShouldTerminateAfterLastWindowClosed`, app encerrava
//      ao fechar Settings.
//    - Iter 2 (commit resetado): AppDelegate + observers `didBecomeKey`/
//      `willClose` filtrados por `window.title == "Torven Settings"`.
//      `didBecomeKey` é post-event → ViewBridge XPC handshake já tinha sido
//      cancelado pelo OS antes do observer rodar. Race condition fundamental.
//    - Iter 3 (commit `dbbd0f1` resetado, Pragmática 1): dropar `LSUIElement`
//      pra app rodar `.regular` desde launch. AppDelegate keep-alive. Smoke
//      Lorenzo: AC-1/2/4/6 PASS mas AC-3 FAIL — app ainda encerrava ao fechar
//      Settings mesmo com `applicationShouldTerminateAfterLastWindowClosed`
//      retornando `false`. Quirk `MenuBarExtra` + `Settings { }` lifecycle
//      em macOS 13+ que AppDelegate adapter não segura.
//
//  Decisão WAVE5-D8-impl-v4 FINAL (Lorenzo + Orion, 2026-06-06): 3 iterações
//  failed em torno da `Settings { }` scene SwiftUI = sinal de design dead-end.
//  Saída: bypassa a scene inteiramente. Mesmo padrão da
//  `MainWindowController` (Story 4.1, Wave 4, já em produção): `NSWindow`
//  programático + `NSHostingView(rootView: SettingsView())`. Zero XPC,
//  zero dependência do SwiftUI App lifecycle pra interception de menu.
//
//  Trade-off aceito: perdemos o "Settings…" item auto-gerado pelo macOS no
//  app menu. Mas com `LSUIElement: true` (preservado iter 4) não há app menu
//  de qualquer jeito — então é zero-loss real. `⌘,` é wired via
//  `KeyboardShortcuts` package (mesmo pattern do `⌘1` da Story 4.1).
//
//  WAVE5-D2 ajustada: Settings continua "não NSPanel" — agora é "NSWindow
//  programmatic wrapping SwiftUI via NSHostingView". Stronger control,
//  zero perda de feature da Story 5.2 (SettingsView SwiftUI preservada
//  integralmente, só o container externo mudou).
//
//  Por que vai funcionar (não-falsificável):
//    - `MainWindowController` já prova o padrão em produção (Story 4.1 Wave 4
//      smoke 37/40 PASS, Wave 4 polish #2 mergeada).
//    - `NSWindow` programático não passa por XPC → `LSUIElement: true` com
//      activation policy `.accessory` não cancela.
//    - `windowShouldClose` retorna `false` + `orderOut(nil)` → state preservado
//      sem dealloc (mesmo pattern do MainWindow).
//

import AppKit
import SwiftUI

final class SettingsWindowController: NSWindowController, NSWindowDelegate {
    static let shared = SettingsWindowController()

    private init() {
        // 520x480 default: maior que os 360pt iniciais da Story 5.2 pra mitigar
        // ISSUE-A inadvertidamente (Wave 5.5 trata sizing dinâmico). A janela
        // não é resizable porque a Settings UI da Story 5.2 tem layout fixo
        // (SecureField grid + OAuth status badges). Adicionar `.resizable` ao
        // styleMask seria possível no futuro se Settings ganhar conteúdo
        // expansível (Wave 7).
        let initialFrame = NSRect(x: 0, y: 0, width: 520, height: 480)
        let window = NSWindow(
            contentRect: initialFrame,
            styleMask: [.titled, .closable, .miniaturizable],
            backing: .buffered,
            defer: false
        )
        window.title = "Torven Settings"
        window.contentView = NSHostingView(rootView: SettingsView())
        // Preserva instance da window e do hosting view (e portanto o estado
        // do SettingsViewModel) quando o user fecha — `windowShouldClose`
        // abaixo intercepta close pra `orderOut` (esconder) em vez de
        // dealloc. Mesmo pattern do MainWindowController.
        window.isReleasedWhenClosed = false
        window.center()

        super.init(window: window)
        window.delegate = self
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("SettingsWindowController does not support NSCoder")
    }

    /// Mostra a janela Settings, trazendo o app ao foreground.
    /// Idempotente — múltiplas chamadas só refocam a janela existente
    /// (e preservam o state do SettingsViewModel via `orderOut`-on-close).
    func show() {
        // `LSUIElement: true` apps precisam `activate` explicitamente pra
        // trazer uma janela ao foco — sem isso a window aparece mas o app
        // não vira o foreground process, então o usuário não consegue
        // digitar nos SecureFields imediatamente.
        NSApp.activate(ignoringOtherApps: true)
        showWindow(nil)
        window?.makeKeyAndOrderFront(nil)
    }

    // MARK: - NSWindowDelegate

    /// Intercepta close (botão vermelho ou ⌘W): esconde em vez de dealloc.
    /// Preserva o state do SettingsViewModel (keys lidas do Keychain,
    /// estado de feedback `.success`/`.error`, etc.) entre aberturas
    /// sucessivas via ⌘,. Mesmo pattern do MainWindowController/Story 4.1.
    func windowShouldClose(_ sender: NSWindow) -> Bool {
        sender.orderOut(nil)
        return false
    }
}
