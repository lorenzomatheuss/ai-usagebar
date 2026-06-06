//
//  MainWindowController.swift
//  Torven
//
//  Story 4.1 (Wave 4): NSWindowController singleton that hosts the Main
//  Window — a document-less utility window for the history view. The
//  shell created here is intentionally empty: Stories 4.2-4.6 will
//  fill MainWindowView with the Date Range Picker and Swift Charts.
//
//  Invocation vectors (decision WAVE4-D1, cravada):
//    - PopoverView footer button "Show History…" (discoverable for new users)
//    - Global hotkey ⌘1 via KeyboardShortcuts SPM (power-user flow)
//
//  Both vectors funnel into MainWindowController.shared.show(). Idempotent
//  by construction: the singleton holds one NSWindow; `show()` only brings
//  it to focus when invoked again.
//
//  Lifecycle: `windowShouldClose` intercepts ⌘W / red-button close and
//  calls `orderOut` instead of deallocating, so reopening preserves window
//  state (position/size via autosaveName "TorvenMainWindow").
//

import AppKit
import SwiftUI

final class MainWindowController: NSWindowController, NSWindowDelegate {
    static let shared = MainWindowController()

    private init() {
        let initialFrame = NSRect(x: 0, y: 0, width: 1040, height: 600)
        let window = NSWindow(
            contentRect: initialFrame,
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = "Torven — History"
        // LAY-001 v2 (Wave 4 polish): top bar in Custom mode needs ~926pt
        // (Period 180 + DatePicker 110 + arrow 15 + DatePicker 110 + Apply 55
        //  + ViewMode 180 + Metric 160 + paddings/spacings). The original
        // 900pt default truncated the Apply button. Bumping default to 1040pt
        // and pinning minSize so neither autosave restoration nor user-resize
        // can drop below the Custom-mode threshold.
        window.minSize = NSSize(width: 1040, height: 480)
        // System persists position/size across launches keyed by this name.
        // minSize is enforced *before* setFrameAutosaveName so any restored
        // frame narrower than 1040pt is clamped up on relaunch.
        window.setFrameAutosaveName("TorvenMainWindow")
        window.contentView = NSHostingView(rootView: MainWindowView())
        window.isReleasedWhenClosed = false
        window.center()

        super.init(window: window)
        window.delegate = self
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("MainWindowController does not support NSCoder")
    }

    /// Shows the Main Window, bringing the app to the foreground if needed.
    /// Idempotent — multiple calls just refocus the existing window.
    func show() {
        // LSUIElement apps must explicitly activate to bring a window to front.
        NSApp.activate(ignoringOtherApps: true)
        showWindow(nil)
        window?.makeKeyAndOrderFront(nil)
    }

    // MARK: - NSWindowDelegate

    /// Intercept close: hide instead of deallocate, so the singleton's
    /// content (and the system-saved frame) is preserved for next open.
    func windowShouldClose(_ sender: NSWindow) -> Bool {
        sender.orderOut(nil)
        return false
    }
}
