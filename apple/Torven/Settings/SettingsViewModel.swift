//
//  SettingsViewModel.swift
//  Torven
//
//  Story 5.2 (Wave 5): owns the Keychain read/write logic for vendor API keys
//  and the OAuth status probes for vendors that authenticate via external CLIs
//  (Anthropic / Claude CLI and OpenAI / Codex CLI — see WAVE5-D7).
//
//  Architecture decisions
//  ----------------------
//  * `@MainActor` because every `@Published` mutation lands on the SwiftUI
//    main thread. The Keychain FFI calls are short (< 100 ms in practice) so
//    `Task { @MainActor in }` wrappers from the View are enough — there is no
//    `Task.detached` here on purpose.
//  * Keys live in `@Published` `@MainActor` state for the lifetime of the
//    Settings window. SecureField masks the visual representation; the
//    plaintext string only exists while the Settings scene is on screen.
//    Closing the window deallocates the ViewModel and the string with it.
//  * The blob shape mirrors `crates/torven-core/src/keychain/mod.rs` —
//    `AccountsBlob { version: 1, accounts: [AccountSecret { account_id,
//    api_key }] }`. Single-account vendors (OpenRouter, Z.AI in Wave 5) use
//    `account_id = "{vendor}-default"` per `config.rs::account_id(vendor,
//    "default")`.
//  * OAuth probes use only `FileManager.fileExists(atPath:)` — never read the
//    JSON content. CLAUDE.md secret-discipline rule: OAuth credential files
//    (`~/.claude/.credentials.json`, `~/.codex/auth.json`) must be treated as
//    opaque (`jq 'keys'` only). We just need to know if the user logged in.
//
//  WAVE5-D7 (cravada @qa 2026-06-06): OpenAI is OAuth-only here, mirroring
//  Anthropic. Story 5.1 dispatches OpenAI via `creds_path: &Path` to the
//  Codex CLI credentials file — there is no API-key code path in the
//  fetcher, so a SecureField for OpenAI would silently do nothing.
//

import Foundation
import SwiftUI

/// Feedback state for a save attempt on an API-key vendor row.
///
/// Modeled as a value type so SwiftUI diffing is cheap. The `.success` case
/// auto-dismisses after 2 s (the View schedules the transition); `.error`
/// persists until the user types again or retries.
enum SaveState: Equatable {
    case idle
    case saving
    case success
    case error(String)
}

/// OAuth status for vendors managed by external CLIs (Anthropic, OpenAI).
///
/// Story 5.2 introduced `.connected` / `.notConfigured`. Story 5.5.2 (Wave
/// 5.5) added `.expired` — Anthropic-only today — so the probe can
/// distinguish a stale token (user has logged in via Claude Code but the
/// access token expired) from a never-configured state. The View renders
/// `.expired` as an orange caution badge prompting "Re-run Claude Code login"
/// instead of misleadingly showing "Connected".
///
/// `notConfigured` is rendered as a neutral hint, not an error — the user
/// hasn't done anything wrong, they just haven't installed the corresponding
/// CLI yet.
enum OAuthStatus: Equatable {
    case connected
    case expired
    case notConfigured
}

@MainActor
final class SettingsViewModel: ObservableObject {
    // MARK: - API key state (OpenRouter, Z.AI — WAVE5-D7)

    /// Editable plaintext key for OpenRouter. Bound to a `SecureField` so it
    /// renders masked. The `@Published` tracks both the initial load from
    /// Keychain and any user edits.
    @Published var openrouterKey: String = ""

    /// Editable plaintext key for Z.AI. Same lifecycle as `openrouterKey`.
    @Published var zaiKey: String = ""

    /// Per-vendor save feedback. Keyed by canonical vendor slug
    /// (`"openrouter"`, `"zai"`) to keep the View dispatch ergonomic.
    @Published var saveState: [String: SaveState] = [
        "openrouter": .idle,
        "zai": .idle,
    ]

    // MARK: - OAuth status state (Anthropic, OpenAI — WAVE5-D7)

    /// Cached probe result for `~/.claude/.credentials.json`. Recomputed in
    /// `refreshOAuthStatus()` on appear.
    @Published var anthropicStatus: OAuthStatus = .notConfigured

    /// Cached probe result for `~/.codex/auth.json`. Same lifecycle as
    /// `anthropicStatus`.
    @Published var openaiStatus: OAuthStatus = .notConfigured

    // MARK: - File-system probe seam (testing hook)

    /// Indirection over `FileManager.default.fileExists(atPath:)` so unit
    /// tests can inject a stub without touching the real home directory.
    /// Defaults to the real `FileManager` in production builds.
    private let fileExists: (String) -> Bool

    /// Resolves the user's home directory. Indirected for the same reason as
    /// `fileExists` — preview/test builds can point at a fixture root.
    private let homeDirectoryProvider: () -> String

    init(
        fileExists: @escaping (String) -> Bool = { FileManager.default.fileExists(atPath: $0) },
        homeDirectoryProvider: @escaping () -> String = { NSHomeDirectory() }
    ) {
        self.fileExists = fileExists
        self.homeDirectoryProvider = homeDirectoryProvider
    }

    // MARK: - OAuth probes

    /// Re-checks both OAuth credentials sources.
    ///
    /// Story 5.5.2 (Wave 5.5): Anthropic now uses an async FFI probe
    /// (`ffiAnthropicOauthStatus()`) that consults the Claude Code Keychain
    /// entry first and the legacy file path second. The Keychain query is
    /// macOS-native (microseconds when cached, milliseconds on first launch
    /// behind a "allow access" prompt), so we run it inside `async` to avoid
    /// stalling the main thread.
    ///
    /// OpenAI stays on the synchronous file probe — Codex CLI writes the
    /// file directly and there's no Keychain story for it yet (WAVE5.5-D9).
    func refreshOAuthStatus() async {
        // OpenAI: synchronous file probe, same as Story 5.2.
        openaiStatus = probe(relativePath: "/.codex/auth.json")

        // Anthropic: async dual-source FFI probe.
        anthropicStatus = await probeAnthropicOAuth()
    }

    /// Bridge to the Rust `ffiAnthropicOauthStatus()` async FFI. Maps the
    /// returned `OAuthStatusFfi` snapshot into the Swift `OAuthStatus` enum.
    ///
    /// The UDL declares this surface as `[Async]` (no `[Throws=...]`), so the
    /// generated signature is `async -> OAuthStatusFfi` (non-throwing). The
    /// Rust side already maps every failure mode to
    /// `isConnected=false, source="none"`, so we never need to surface a raw
    /// error to the user — just project the snapshot into the enum.
    private func probeAnthropicOAuth() async -> OAuthStatus {
        let status: OAuthStatusFfi = await ffiAnthropicOauthStatus()

        if !status.isConnected {
            return .notConfigured
        }
        if status.isExpired {
            return .expired
        }
        return .connected
    }

    private func probe(relativePath: String) -> OAuthStatus {
        let fullPath = homeDirectoryProvider() + relativePath
        return fileExists(fullPath) ? .connected : .notConfigured
    }

    // MARK: - Keychain load

    /// Loads existing keys from the Keychain for both API-key vendors. Errors
    /// are swallowed silently — a missing entry simply leaves the field empty
    /// so the user can paste a key for the first time.
    ///
    /// The FFI signature is `ffiKeychainGetBlob(vendor:) throws -> [UInt8]`.
    /// `KeychainFfiError.Storage` is thrown when the keychain entry doesn't
    /// exist; we treat that as "no key yet" rather than surfacing the error.
    func loadKeysFromKeychain() {
        openrouterKey = readKey(vendor: "openrouter") ?? ""
        zaiKey = readKey(vendor: "zai") ?? ""
    }

    private func readKey(vendor: String) -> String? {
        let blob: [UInt8]
        do {
            blob = try ffiKeychainGetBlob(vendor: vendor)
        } catch {
            // CredentialMissing → expected first-run state. Any other storage
            // error also falls back to "no key" so the user can still paste a
            // new one. We intentionally do NOT log the error message because
            // some Keychain errors include account_id values that we treat
            // as low-sensitivity but still avoid in user-visible logs.
            return nil
        }

        let data = Data(blob)
        guard let decoded = try? JSONDecoder().decode(KeychainBlob.self, from: data),
              let firstAccount = decoded.accounts.first
        else {
            return nil
        }
        return firstAccount.apiKey
    }

    // MARK: - Keychain save

    /// Persists a single-account blob to the Keychain. The save is `async` so
    /// the UI doesn't block on the FFI call (Keychain writes on macOS can
    /// trigger a "allow access" prompt the first time, which takes ~1 s).
    ///
    /// `vendor` MUST be the canonical slug (`"openrouter"` or `"zai"`).
    /// Whitespace trim happens here (AC-9) so the caller doesn't have to
    /// remember.
    func saveKey(vendor: String, key: String) async {
        let trimmed = key.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            // AC-8: empty input is ignored, not surfaced as an error. The
            // button is disabled in the View, so this branch only fires if
            // someone wires the API around the button (defensive).
            saveState[vendor] = .idle
            return
        }

        saveState[vendor] = .saving

        let blob = KeychainBlob(
            version: 1,
            accounts: [
                KeychainBlobEntry(accountId: "\(vendor)-default", apiKey: trimmed),
            ]
        )

        do {
            let bytes = try JSONEncoder().encode(blob)
            try ffiKeychainSetBlob(vendor: vendor, blob: Array(bytes))
            saveState[vendor] = .success

            // AC-4: auto-dismiss the checkmark after 2 s so the row returns
            // to a neutral state. Capture self weakly to avoid retaining the
            // ViewModel past Settings-window close.
            Task { [weak self] in
                try? await Task.sleep(nanoseconds: 2_000_000_000)
                // Only clear if still showing success — the user may have
                // triggered another save in the meantime.
                if self?.saveState[vendor] == .success {
                    self?.saveState[vendor] = .idle
                }
            }
        } catch let error as KeychainFfiError {
            // KeychainFfiError.Storage carries a message; the message itself
            // doesn't include the api_key (we control the Rust side), so it's
            // safe to surface as a short hint.
            switch error {
            case .Storage(let message):
                saveState[vendor] = .error(message)
            }
        } catch {
            // JSON encoding failure or unexpected FFI error. Generic message
            // — never include the key in the visible string.
            saveState[vendor] = .error("Erro ao salvar — tente novamente.")
        }
    }
}

// MARK: - Blob shape (mirrors crates/torven-core/src/keychain/mod.rs)

/// Versioned blob format owned by the Rust core. Keep the field names in sync
/// with `AccountsBlob` / `AccountSecret` in `keychain/mod.rs` — divergence
/// here means `ffi_refresh_vendor` won't find the key and Refresh silently
/// uses no credentials.
///
/// Note: Rust's `serde` uses snake_case by default; we mirror that via
/// explicit `CodingKeys` so Swift's idiomatic camelCase doesn't leak into the
/// wire format.
private struct KeychainBlob: Codable {
    let version: UInt32
    let accounts: [KeychainBlobEntry]
}

private struct KeychainBlobEntry: Codable {
    let accountId: String
    let apiKey: String

    private enum CodingKeys: String, CodingKey {
        case accountId = "account_id"
        case apiKey = "api_key"
    }
}
