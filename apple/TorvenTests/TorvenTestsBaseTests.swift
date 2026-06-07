//
//  TorvenTestsBaseTests.swift
//  TorvenTests
//
//  Story 5.5.3 (Wave 5.5) — Phase 4 (AC-9 / AC-10 / AC-14).
//
//  Smoke tests for the shared fixture helpers in `TorvenTestsBase.swift`.
//  WAVE5.5-D14 cravou cobertura inicial = 15 tests (5 Settings + 8 Chart +
//  2 helper utilities). These two tests round out the count and protect the
//  fixture builders against drift — if a future FFI struct grows a new
//  field, the matching `makeFixture*` factory must keep compiling, and
//  these tests are what catch a silent regression.
//

@testable import Torven
import XCTest

final class TorvenTestsBaseTests: TorvenTestCase {
    // MARK: - Default-fixture shape (AC-9 helper smoke)

    /// `makeFixtureTimeBucket()` with all default arguments must return a
    /// `TimeBucket` shaped exactly like the production FFI emits (Story
    /// 4.0.5). Field-by-field assertion catches drift early.
    func test_makeFixtureTimeBucket_default_values_match_documented_shape() {
        let bucket = makeFixtureTimeBucket()

        XCTAssertEqual(bucket.bucketStartTs, 1_748_736_000_000,
                       "Default bucketStartTs should be the documented 2026-06-01 anchor")
        XCTAssertEqual(bucket.bucketEndTs, 1_748_739_600_000,
                       "Default bucketEndTs should be one hour after bucketStartTs")
        XCTAssertEqual(bucket.vendor, "openrouter")
        XCTAssertEqual(bucket.accountId, "openrouter-default")
        XCTAssertEqual(bucket.costSumUsd, 0.42, accuracy: 0.001)
        XCTAssertEqual(bucket.tokensSum, 12_345)
        XCTAssertEqual(bucket.requestCount, 7)
        XCTAssertEqual(bucket.metricKind, "spend")
    }

    /// `makeFixtureBudgetStatus()` defaults to a configured-budget state so
    /// the most common test case ("user has a budget") doesn't need to
    /// override any argument. Verify the default `hasBudget` is `true` —
    /// flipping this default unannounced would break Wave 5.5+ tests
    /// silently.
    func test_makeFixtureBudgetStatus_default_hasBudget_true() {
        let budget = makeFixtureBudgetStatus()

        XCTAssertTrue(budget.hasBudget,
                      "Default fixture must represent a configured-budget user")
        XCTAssertEqual(budget.totalSpentUsd, 12.34, accuracy: 0.001)
        XCTAssertEqual(budget.totalBudgetUsd, 50.0)
        XCTAssertEqual(budget.totalPercentUsed, 24.68, accuracy: 0.001)
        XCTAssertTrue(budget.perVendor.isEmpty,
                      "Default fixture has no per-vendor entries — pass `perVendor:` to override")
    }
}
