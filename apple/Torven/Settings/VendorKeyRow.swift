//
//  VendorKeyRow.swift
//  Torven
//
//  Story 5.2 (Wave 5): per-vendor row inside the SettingsView Form. Two
//  variants share the same outer container so the visual rhythm of the form
//  stays consistent:
//
//    1. `.apiKey` — SecureField + Save button + feedback (OpenRouter, Z.AI)
//    2. `.oauthStatus` — read-only badge with connection hint (Anthropic,
//       OpenAI — see WAVE5-D7)
//
//  WAVE5-D7 (cravada @qa 2026-06-06): the OpenAI row is `.oauthStatus`, not
//  `.apiKey`. The Rust fetcher for OpenAI consumes a credentials path, not an
//  api_key string, so a SecureField here would be a dead UI element.
//

import SwiftUI

/// Discriminator for the two row variants. Modeled as an enum (not two
/// separate views) so the parent Form can iterate uniformly if a future
/// story wants a `ForEach` over a `[VendorRowModel]`.
enum VendorRowVariant {
    /// API-key variant: editable SecureField persisted to the Keychain.
    case apiKey(
        keyBinding: Binding<String>,
        saveState: SaveState,
        onSave: () -> Void
    )

    /// OAuth status variant: read-only label reflecting whether the external
    /// CLI has logged in. The `cliName` is shown in the hint string
    /// (e.g. "Claude CLI", "Codex CLI").
    case oauthStatus(status: OAuthStatus, cliName: String)
}

struct VendorKeyRow: View {
    let vendorDisplayName: String
    let variant: VendorRowVariant

    var body: some View {
        Section(vendorDisplayName) {
            switch variant {
            case let .apiKey(keyBinding, saveState, onSave):
                apiKeyContent(keyBinding: keyBinding, saveState: saveState, onSave: onSave)
            case let .oauthStatus(status, cliName):
                oauthStatusContent(status: status, cliName: cliName)
            }
        }
    }

    // MARK: - API-key variant (OpenRouter, Z.AI)

    @ViewBuilder
    private func apiKeyContent(
        keyBinding: Binding<String>,
        saveState: SaveState,
        onSave: @escaping () -> Void
    ) -> some View {
        SecureField("API Key", text: keyBinding)
            .textFieldStyle(.roundedBorder)
            // Disabling autocorrect/capitalization avoids the macOS text
            // engine helpfully "fixing" an `sk-…` key. SecureField masks the
            // display but the underlying string still passes through the
            // input pipeline. Story 5.5.1 (MNT-001) dropped the legacy
            // `.disableAutocorrection(true)` call: it was deprecated in
            // macOS 14 in favor of `.autocorrectionDisabled(true)` and
            // produced a build warning on every clean build.
            .autocorrectionDisabled(true)

        HStack(spacing: 12) {
            Button("Salvar") {
                onSave()
            }
            .disabled(isSaveDisabled(keyBinding.wrappedValue, saveState: saveState))

            saveFeedback(for: saveState)
            Spacer(minLength: 0)
        }
    }

    private func isSaveDisabled(_ key: String, saveState: SaveState) -> Bool {
        // AC-8: empty / whitespace-only keys can't be saved.
        if key.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return true
        }
        // Prevent double-click during in-flight save.
        if saveState == .saving {
            return true
        }
        return false
    }

    @ViewBuilder
    private func saveFeedback(for state: SaveState) -> some View {
        switch state {
        case .idle:
            EmptyView()

        case .saving:
            HStack(spacing: 6) {
                ProgressView()
                    .controlSize(.small)
                Text("Salvando…")
                    .foregroundColor(.secondary)
                    .font(.caption)
            }

        case .success:
            HStack(spacing: 6) {
                Image(systemName: "checkmark.circle.fill")
                    .foregroundColor(.green)
                Text("Salvo")
                    .foregroundColor(.green)
                    .font(.caption)
            }
            // The 2 s auto-dismiss is driven by SettingsViewModel; the View
            // simply reflects whatever the published state currently is.
            .transition(.opacity)

        case .error(let message):
            HStack(spacing: 6) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundColor(.red)
                Text(message)
                    .foregroundColor(.red)
                    .font(.caption)
                    .lineLimit(1)
                    .truncationMode(.tail)
            }
        }
    }

    // MARK: - OAuth status variant (Anthropic, OpenAI)

    @ViewBuilder
    private func oauthStatusContent(status: OAuthStatus, cliName: String) -> some View {
        HStack(spacing: 8) {
            switch status {
            case .connected:
                Image(systemName: "checkmark.circle.fill")
                    .foregroundColor(.green)
                Text("Conectado via \(cliName)")
                    .foregroundColor(.primary)
            case .expired:
                // Story 5.5.2 (Wave 5.5): the user IS logged in (we found a
                // credentials source) but the access token's `expiresAt` is
                // in the past. Prompt them to refresh via the CLI rather
                // than misleadingly showing "Connected".
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundColor(.orange)
                Text("Token expirado — refaça login no \(cliName)")
                    .foregroundColor(.orange)
                    .lineLimit(2)
                    .fixedSize(horizontal: false, vertical: true)
            case .notConfigured:
                Image(systemName: "info.circle")
                    .foregroundColor(.secondary)
                Text("Não configurado — instale o \(cliName) e faça login")
                    .foregroundColor(.secondary)
                    .lineLimit(2)
                    .fixedSize(horizontal: false, vertical: true)
            }
            Spacer(minLength: 0)
        }
    }
}

#Preview("API key — idle") {
    Form {
        VendorKeyRow(
            vendorDisplayName: "OpenRouter",
            variant: .apiKey(
                keyBinding: .constant(""),
                saveState: .idle,
                onSave: {}
            )
        )
    }
    .frame(width: 480, height: 160)
}

#Preview("API key — success") {
    Form {
        VendorKeyRow(
            vendorDisplayName: "Z.AI",
            variant: .apiKey(
                keyBinding: .constant("sk-test-1234"),
                saveState: .success,
                onSave: {}
            )
        )
    }
    .frame(width: 480, height: 160)
}

#Preview("OAuth — connected") {
    Form {
        VendorKeyRow(
            vendorDisplayName: "Anthropic",
            variant: .oauthStatus(status: .connected, cliName: "Claude CLI")
        )
    }
    .frame(width: 480, height: 120)
}

#Preview("OAuth — not configured") {
    Form {
        VendorKeyRow(
            vendorDisplayName: "OpenAI",
            variant: .oauthStatus(status: .notConfigured, cliName: "Codex CLI")
        )
    }
    .frame(width: 480, height: 120)
}

#Preview("OAuth — expired (Story 5.5.2)") {
    Form {
        VendorKeyRow(
            vendorDisplayName: "Anthropic",
            variant: .oauthStatus(status: .expired, cliName: "Claude Code")
        )
    }
    .frame(width: 480, height: 120)
}
