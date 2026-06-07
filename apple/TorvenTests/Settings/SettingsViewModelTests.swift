//
//  SettingsViewModelTests.swift
//  TorvenTests
//
//  Story 5.5.3 (Wave 5.5) — Phase 2 (AC-5 / AC-6).
//
//  Covers the `refreshOAuthStatus()` mapping logic introduced in Story 5.2
//  and extended in Story 5.5.2 (`.expired` enum case). The tests use the DI
//  seams added to `SettingsViewModel.init` (`fileExists`, `homeDirectoryProvider`,
//  `anthropicOAuthProvider`) so no real Keychain or filesystem state is
//  touched at test time.
//
//  ## Test inventory
//
//  1. test_refreshOAuthStatus_anthropic_connected_when_keychain_present
//  2. test_refreshOAuthStatus_anthropic_notConfigured_when_keychain_missing
//  3. test_refreshOAuthStatus_anthropic_expired_when_token_past_expiry  (5.5.2 dep)
//  4. test_refreshOAuthStatus_openai_connected_when_file_exists
//  5. test_refreshOAuthStatus_openai_notConfigured_when_file_missing
//
//  ## Why `@MainActor` on the test methods?
//
//  `SettingsViewModel` is `@MainActor`. Reading `anthropicStatus` /
//  `openaiStatus` from a non-MainActor context would either crash at runtime
//  or trigger Swift 6 strict-concurrency warnings. Each `test_*` is marked
//  `@MainActor` so the access is statically safe.
//

@testable import Torven
import XCTest

@MainActor
final class SettingsViewModelTests: TorvenTestCase {
    // MARK: - Anthropic (AC-5 tests 1-3)

    /// `.connected` when the FFI returns a healthy snapshot
    /// (isConnected=true, isExpired=false). The default fixture from
    /// `makeFixtureAnthropicOAuthStatus()` represents this state.
    func test_refreshOAuthStatus_anthropic_connected_when_keychain_present() async {
        let viewModel = SettingsViewModel(
            fileExists: { _ in false },          // OpenAI path absent — irrelevant here
            homeDirectoryProvider: { "/tmp/test-home" },
            anthropicOAuthProvider: {
                makeFixtureAnthropicOAuthStatus(
                    isConnected: true,
                    isExpired: false,
                    source: "keychain"
                )
            }
        )

        await viewModel.refreshOAuthStatus()

        XCTAssertEqual(viewModel.anthropicStatus, .connected,
                       "Anthropic should map to .connected when FFI reports isConnected=true && isExpired=false")
    }

    /// `.notConfigured` when the FFI returns an empty snapshot
    /// (isConnected=false). Mirrors the case where Claude Code is not
    /// installed or the user has never authenticated.
    func test_refreshOAuthStatus_anthropic_notConfigured_when_keychain_missing() async {
        let viewModel = SettingsViewModel(
            fileExists: { _ in false },
            homeDirectoryProvider: { "/tmp/test-home" },
            anthropicOAuthProvider: {
                makeFixtureAnthropicOAuthStatus(
                    isConnected: false,
                    isExpired: false,
                    expiresAtSecs: nil,
                    source: "none"
                )
            }
        )

        await viewModel.refreshOAuthStatus()

        XCTAssertEqual(viewModel.anthropicStatus, .notConfigured,
                       "Anthropic should map to .notConfigured when FFI reports isConnected=false")
    }

    /// `.expired` when the FFI returns a snapshot with isConnected=true but
    /// isExpired=true. This branch was added in Story 5.5.2 — without
    /// the `.expired` enum case, this test would not compile.
    func test_refreshOAuthStatus_anthropic_expired_when_token_past_expiry() async {
        let expiredAt: Int64 = Int64(Date().timeIntervalSince1970) - 3_600  // 1h ago
        let viewModel = SettingsViewModel(
            fileExists: { _ in false },
            homeDirectoryProvider: { "/tmp/test-home" },
            anthropicOAuthProvider: {
                makeFixtureAnthropicOAuthStatus(
                    isConnected: true,
                    isExpired: true,
                    expiresAtSecs: expiredAt,
                    source: "keychain"
                )
            }
        )

        await viewModel.refreshOAuthStatus()

        XCTAssertEqual(viewModel.anthropicStatus, .expired,
                       "Anthropic should map to .expired when FFI reports isConnected=true && isExpired=true")
    }

    // MARK: - OpenAI (AC-5 tests 4-5)

    /// `.connected` when `~/.codex/auth.json` exists. The seam returns
    /// `true` only for the OpenAI probe path so we can assert that the
    /// ViewModel actually consults the correct file path (not just any
    /// file).
    func test_refreshOAuthStatus_openai_connected_when_file_exists() async {
        var probedPaths: [String] = []
        let viewModel = SettingsViewModel(
            fileExists: { path in
                probedPaths.append(path)
                return path.hasSuffix("/.codex/auth.json")
            },
            homeDirectoryProvider: { "/tmp/test-home" },
            anthropicOAuthProvider: {
                makeFixtureAnthropicOAuthStatus(isConnected: false, source: "none")
            }
        )

        await viewModel.refreshOAuthStatus()

        XCTAssertEqual(viewModel.openaiStatus, .connected,
                       "OpenAI should map to .connected when the codex auth file exists")
        XCTAssertTrue(probedPaths.contains("/tmp/test-home/.codex/auth.json"),
                      "OpenAI probe must read the canonical /.codex/auth.json path. Probed paths: \(probedPaths)")
    }

    /// `.notConfigured` when the OpenAI credentials file is absent —
    /// expected first-run state for users who haven't installed Codex CLI.
    func test_refreshOAuthStatus_openai_notConfigured_when_file_missing() async {
        let viewModel = SettingsViewModel(
            fileExists: { _ in false },
            homeDirectoryProvider: { "/tmp/test-home" },
            anthropicOAuthProvider: {
                makeFixtureAnthropicOAuthStatus(isConnected: false, source: "none")
            }
        )

        await viewModel.refreshOAuthStatus()

        XCTAssertEqual(viewModel.openaiStatus, .notConfigured,
                       "OpenAI should map to .notConfigured when /.codex/auth.json is absent")
    }
}
