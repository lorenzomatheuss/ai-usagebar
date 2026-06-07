//
//  TorvenTestsBase.swift
//  TorvenTests
//
//  Story 5.5.3 (Wave 5.5): shared base class + fixture helpers consumed by
//  all `*ViewModelTests` files. Centralising fixture construction here keeps
//  individual test files focused on intent (Given/When/Then) instead of FFI
//  struct memberwise-init noise.
//
//  ## Why a `TorvenTestCase` subclass?
//
//  Two reasons:
//
//  1. **Future setup hook (AC-9).** The story spec wants a place to add
//     "common cleanup" without touching every test file. `TorvenTestCase`'s
//     `setUp()` is currently a thin pass-through, but new test groups
//     (Wave 7.5 MenuBarStateProviderTests, future SettingsViewModelTests
//     extensions) get the cleanup for free if they subclass.
//
//  2. **MainActor isolation hygiene.** SwiftUI ViewModels under test
//     (`SettingsViewModel`, `ChartViewModel`) are `@MainActor`. Tests that
//     touch them need to hop to the main actor. WAVE5.5-D12 cravou DI sem
//     library — so we use plain `@MainActor func test_*() async` per test;
//     `TorvenTestCase` doesn't force a particular pattern, it just provides
//     the helpers.
//
//  ## Fixtures (AC-9)
//
//  The factory functions below return ready-to-use values for the FFI types
//  declared by `torven_core.swift`. Defaults are chosen so the caller only
//  has to specify the fields it cares about (e.g. `expiresAt:`).
//

import Foundation
@testable import Torven
import XCTest

/// Shared XCTestCase subclass for Torven unit tests.
///
/// Override-friendly: subclasses can call `super.setUp()`/`super.tearDown()`
/// to inherit the common hooks while adding their own setup. Wave 5.5 keeps
/// the base intentionally minimal — Wave 7.5 may add per-test snapshot
/// resets when MenuBarStateProvider lands.
class TorvenTestCase: XCTestCase {
    override func setUp() {
        super.setUp()
        // Reserved for future shared setup (e.g. resetting a process-level
        // FFI slot before each test). No-op in Wave 5.5.
    }

    override func tearDown() {
        super.tearDown()
        // Reserved for future shared teardown. No-op in Wave 5.5.
    }
}

// MARK: - TimeBucket fixture (AC-9)

/// Builds a `TimeBucket` with reasonable defaults so call sites only specify
/// the fields they care about. The default `bucketStartTs`/`bucketEndTs` pair
/// is 2026-06-01 00:00..01:00 UTC in milliseconds — matches the unit the
/// Rust `ffi_query_aggregated` returns (millis-since-epoch, see
/// `DateRange.sinceTs` / `.untilTs` in `DateRangePicker.swift`).
func makeFixtureTimeBucket(
    bucketStartTs: Int64 = 1_748_736_000_000,
    bucketEndTs: Int64 = 1_748_739_600_000,
    vendor: String = "openrouter",
    accountId: String? = "openrouter-default",
    costSumUsd: Double = 0.42,
    tokensSum: UInt64 = 12_345,
    requestCount: UInt32 = 7,
    metricKind: String = "spend"
) -> TimeBucket {
    TimeBucket(
        bucketStartTs: bucketStartTs,
        bucketEndTs: bucketEndTs,
        vendor: vendor,
        accountId: accountId,
        costSumUsd: costSumUsd,
        tokensSum: tokensSum,
        requestCount: requestCount,
        metricKind: metricKind
    )
}

// MARK: - BudgetStatus fixture (AC-9)

/// Builds a `BudgetStatus` with `hasBudget = true` by default so callers
/// testing the "budget configured" path don't have to wire a list of
/// `VendorBudgetStatus`. Pass `hasBudget: false` to test the
/// "no budget configured" code path (`BudgetBurn` renders `EmptyView()`).
func makeFixtureBudgetStatus(
    totalSpentUsd: Double = 12.34,
    totalBudgetUsd: Double? = 50.0,
    totalPercentUsed: Double = 24.68,
    perVendor: [VendorBudgetStatus] = [],
    hasBudget: Bool = true
) -> BudgetStatus {
    BudgetStatus(
        totalSpentUsd: totalSpentUsd,
        totalBudgetUsd: totalBudgetUsd,
        totalPercentUsed: totalPercentUsed,
        perVendor: perVendor,
        hasBudget: hasBudget
    )
}

// MARK: - Anthropic OAuth probe fixtures (AC-9)

/// Builds an `OAuthStatusFfi` snapshot mirroring the Rust side
/// `ffi_anthropic_oauth_status()` shape. Defaults to a healthy keychain
/// snapshot expiring in one hour from "now"; tests for the `.expired`
/// branch should pass `expiresAtSecs:` in the past with `isExpired: true`.
func makeFixtureAnthropicOAuthStatus(
    isConnected: Bool = true,
    isExpired: Bool = false,
    expiresAtSecs: Int64? = nil,
    source: String = "keychain"
) -> OAuthStatusFfi {
    let resolvedExpiry: Int64? = expiresAtSecs ?? (isConnected ? Int64(Date().timeIntervalSince1970) + 3_600 : nil)
    return OAuthStatusFfi(
        isConnected: isConnected,
        isExpired: isExpired,
        expiresAtSecs: resolvedExpiry,
        source: source
    )
}

// MARK: - Claude Code keychain blob fixture (AC-9)

/// Returns a JSON blob shaped like the Claude Code keychain entry that
/// `crates/torven-core/src/anthropic/oauth.rs` parses (Story 5.5.2). The
/// `expiresAt` parameter governs whether the blob represents a "valid" or
/// "expired" snapshot — the Rust side compares against `SystemTime::now()`.
///
/// Schema (intentionally minimal — the parser tolerates unknown fields):
///   `{"claudeAiOauth": {"accessToken": "...", "expiresAt": <ms>, "refreshToken": "...", "scopes": [...] }}`
func makeFixtureClaudeAiOauth(
    expiresAt: Int64,
    accessToken: String = "sk-ant-test-access-token-fixture",
    refreshToken: String = "sk-ant-test-refresh-token-fixture",
    scopes: [String] = ["user:inference", "user:profile"]
) -> Data {
    let payload: [String: Any] = [
        "claudeAiOauth": [
            "accessToken": accessToken,
            "expiresAt": expiresAt,
            "refreshToken": refreshToken,
            "scopes": scopes,
        ],
    ]
    // swiftlint:disable:next force_try
    return try! JSONSerialization.data(withJSONObject: payload, options: [.sortedKeys])
}
