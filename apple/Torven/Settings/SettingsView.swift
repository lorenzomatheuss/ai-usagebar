//
//  SettingsView.swift
//  Torven
//
//  Story 5.2 (Wave 5): root SwiftUI Settings scene. Registered in
//  `TorvenApp.swift` via the modern `Settings { SettingsView() }` scene
//  (WAVE5-D2). The OS wires Cmd+, to "Settings…" automatically; no
//  AppDelegate plumbing required.
//
//  Layout
//  ------
//  * SwiftUI `Form` with four `Section`s, one per vendor, in deterministic
//    order: Anthropic, OpenAI, OpenRouter, Z.AI. The order matches the
//    canonical `getVendorList()` output minus the ones we don't ship in
//    Wave 5 (OpenRouter and Z.AI are the only API-key vendors per WAVE5-D7).
//  * Each section is a `VendorKeyRow` with one of two variants:
//      - `.oauthStatus` for Anthropic / OpenAI (read-only badge)
//      - `.apiKey`      for OpenRouter / Z.AI  (SecureField + Save)
//
//  Sizing
//  ------
//  Story 5.5.1 (Wave 5.5 / WAVE5.5-D2): dynamic min/ideal sizing replaces the
//  legacy `.frame(width: 520, height: 360)` from Story 5.2. The original
//  hardcoded 360pt height clipped the Z.AI section once we had 4 vendor rows
//  + Budget section visible. We now declare `minWidth: 520, idealWidth: 520,
//  minHeight: 480, idealHeight: 520` so SwiftUI sizes the window to fit the
//  Form content while still preventing the user from collapsing the window
//  below the readability floor. `SettingsWindowController` (Story 5.2.1
//  iteration 4) creates the NSWindow at the matching default 520x480.
//

import SwiftUI

struct SettingsView: View {
    @StateObject private var viewModel = SettingsViewModel()

    var body: some View {
        Form {
            VendorKeyRow(
                vendorDisplayName: "Anthropic",
                variant: .oauthStatus(
                    status: viewModel.anthropicStatus,
                    // Story 5.5.2: Claude Code (Keychain) is preferred over
                    // legacy Claude CLI standalone. Both are supported via
                    // the dual-source resolver but messaging defaults to
                    // Claude Code since that's the modern flow.
                    cliName: "Claude Code"
                )
            )

            VendorKeyRow(
                vendorDisplayName: "OpenAI",
                variant: .oauthStatus(
                    status: viewModel.openaiStatus,
                    cliName: "Codex CLI"
                )
            )

            VendorKeyRow(
                vendorDisplayName: "OpenRouter",
                variant: .apiKey(
                    keyBinding: $viewModel.openrouterKey,
                    saveState: viewModel.saveState["openrouter"] ?? .idle,
                    onSave: {
                        Task { await viewModel.saveKey(
                            vendor: "openrouter",
                            key: viewModel.openrouterKey
                        ) }
                    }
                )
            )

            VendorKeyRow(
                vendorDisplayName: "Z.AI",
                variant: .apiKey(
                    keyBinding: $viewModel.zaiKey,
                    saveState: viewModel.saveState["zai"] ?? .idle,
                    onSave: {
                        Task { await viewModel.saveKey(
                            vendor: "zai",
                            key: viewModel.zaiKey
                        ) }
                    }
                )
            )
        }
        .formStyle(.grouped)
        .frame(minWidth: 520, idealWidth: 520, minHeight: 480, idealHeight: 520)
        .animation(.easeInOut(duration: 0.2), value: viewModel.saveState)
        .onAppear {
            // Story 5.5.2 (Wave 5.5): OAuth probe is now async — the
            // Anthropic path consults the Keychain entry `"Claude
            // Code-credentials"` via FFI, which can block briefly on first
            // launch (macOS "allow access" prompt). OpenAI stays on the
            // synchronous file probe inside the same async block to keep
            // the call-site uniform.
            Task { @MainActor in
                await viewModel.refreshOAuthStatus()
            }

            // Keychain reads (API-key vendors — OpenRouter, Z.AI) can also
            // block briefly on first launch. Defer to a Task so the window
            // draws immediately with empty fields, then populates.
            Task { @MainActor in
                viewModel.loadKeysFromKeychain()
            }
        }
    }
}

#Preview {
    SettingsView()
}
