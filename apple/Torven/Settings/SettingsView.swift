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
//  `.frame(width: 520, height: 360)` keeps the window comfortable on macOS
//  13+ without forcing a resize when the user runs at the default 1.0 text
//  scale. The dimensions are hardcoded because Settings windows on macOS
//  conventionally do not resize.
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
                    cliName: "Claude CLI"
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
        .frame(width: 520, height: 360)
        .animation(.easeInOut(duration: 0.2), value: viewModel.saveState)
        .onAppear {
            // Synchronous OAuth probe — sub-millisecond on local FS, so
            // running it on the main thread before the first frame is the
            // simpler & correct choice (no flash of "Not configured").
            viewModel.refreshOAuthStatus()

            // Keychain reads can block briefly on first launch (macOS may
            // surface a "allow access" prompt). Defer to a Task so the
            // window draws immediately with empty fields, then populates.
            Task { @MainActor in
                viewModel.loadKeysFromKeychain()
            }
        }
    }
}

#Preview {
    SettingsView()
}
