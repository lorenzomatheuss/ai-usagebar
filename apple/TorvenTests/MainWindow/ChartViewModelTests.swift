//
//  ChartViewModelTests.swift
//  TorvenTests
//
//  Story 5.5.3 (Wave 5.5) — Phase 3 (AC-7 / AC-8).
//
//  Validates the reload + refresh business logic against the FFI surface
//  exposed via `ChartViewModel`'s constructor injection seams (`refreshFn`,
//  `queryFn`, `budgetFn`, `initHistoryFn`).
//
//  ## Test inventory (8)
//
//  Reload (3):
//   1. test_reload_populates_aggregatedData_from_ffi_query
//   2. test_reload_sets_budget_status_from_ffi_call
//   3. test_reload_handles_empty_buckets_gracefully
//
//  Refresh (5):
//   4. test_refresh_calls_ffi_refresh_for_each_vendor
//   5. test_refresh_swallows_credentialMissing_silently
//   6. test_refresh_aggregates_refreshErrors_for_non_credential_errors
//   7. test_refresh_sets_isRefreshing_true_during_inflight_false_after
//   8. test_refresh_calls_reload_after_completion
//
//  ## Concurrency notes
//
//  `ChartViewModel` is `@MainActor`. All tests use `@MainActor` to access
//  `@Published` state without races. The `awaitInitialReload(...)` helper
//  yields the runloop a few times so the Combine subscriber set up in
//  `init` has a chance to dispatch the first `reload(for:)` — without this,
//  asserting on `chartData` immediately after init would race the
//  `.sink { ... }` invocation.
//

import Combine
@testable import Torven
import XCTest

@MainActor
final class ChartViewModelTests: TorvenTestCase {
    // MARK: - Helpers

    /// Spin the runloop briefly so Combine subscribers (and any inflight
    /// `Task { ... }` work scheduled in `reload(for:)`) get a chance to
    /// complete before the test reads `@Published` state.
    private func waitForInflightWork(iterations: Int = 50) async {
        for _ in 0..<iterations {
            await Task.yield()
        }
    }

    /// Builds a `ChartViewModel` with all FFI seams stubbed. The default
    /// `queryFn` returns an empty bucket array (matching the "no history"
    /// path) and `budgetFn` returns a `hasBudget == false` snapshot — both
    /// are no-ops for the production state, so individual tests only need
    /// to override the seams they care about.
    private func makeSubject(
        initialRange: DateRange = .last7Days(),
        refreshFn: @escaping ChartViewModel.RefreshFn = { _ in },
        queryFn: @escaping ChartViewModel.QueryFn = { _, _, _, _, _, _ in [] },
        budgetFn: @escaping ChartViewModel.BudgetFn = {
            makeFixtureBudgetStatus(
                totalSpentUsd: 0.0,
                totalBudgetUsd: nil,
                totalPercentUsed: 0.0,
                hasBudget: false
            )
        },
        initHistoryFn: @escaping ChartViewModel.InitHistoryFn = { _ in }
    ) -> (ChartViewModel, MainWindowViewModel) {
        let main = MainWindowViewModel(initialRange: initialRange)
        let chart = ChartViewModel(
            mainViewModel: main,
            refreshFn: refreshFn,
            queryFn: queryFn,
            budgetFn: budgetFn,
            initHistoryFn: initHistoryFn
        )
        return (chart, main)
    }

    // MARK: - Reload (AC-7 tests 1-3)

    /// The Combine sink fires once on init with the seed `initialRange`,
    /// triggering `reload(for:)`. The stubbed `queryFn` returns a single
    /// `TimeBucket`; we assert `chartData.samples` contains exactly that
    /// vendor.
    func test_reload_populates_aggregatedData_from_ffi_query() async {
        let bucket = makeFixtureTimeBucket(vendor: "openrouter", costSumUsd: 1.23)
        let (chart, _) = makeSubject(
            queryFn: { _, _, _, _, _, _ in [bucket] }
        )

        await waitForInflightWork()

        XCTAssertEqual(chart.chartData.samples.count, 1,
                       "Expected one sample mapped from the stubbed bucket")
        XCTAssertEqual(chart.chartData.samples.first?.vendor, "openrouter")
        XCTAssertFalse(chart.chartData.isEmpty,
                       "chartData should not report empty when buckets are present")
        XCTAssertNil(chart.errorMessage, "Successful reload must clear errorMessage")
    }

    /// `reload(for:)` always re-fetches the budget alongside the chart
    /// buckets. We stub a `hasBudget=true` fixture and assert the seed
    /// `hasBudget=false` was overwritten.
    func test_reload_sets_budget_status_from_ffi_call() async {
        let budget = makeFixtureBudgetStatus(
            totalSpentUsd: 12.34,
            totalBudgetUsd: 50.0,
            totalPercentUsed: 24.68,
            hasBudget: true
        )
        let (chart, _) = makeSubject(
            budgetFn: { budget }
        )

        await waitForInflightWork()

        XCTAssertTrue(chart.budgetStatus.hasBudget,
                      "budgetStatus.hasBudget should reflect the FFI call (was hasBudget=false at seed)")
        XCTAssertEqual(chart.budgetStatus.totalSpentUsd, 12.34, accuracy: 0.001)
        XCTAssertEqual(chart.budgetStatus.totalBudgetUsd, 50.0)
    }

    /// Empty bucket array → `chartData.isEmpty == true`. The view leans on
    /// this signal to render the empty-state path (AC-4 of Story 4.3).
    func test_reload_handles_empty_buckets_gracefully() async {
        let (chart, _) = makeSubject(
            queryFn: { _, _, _, _, _, _ in [] }
        )

        await waitForInflightWork()

        XCTAssertTrue(chart.chartData.isEmpty,
                      "Empty bucket array should produce an empty ChartData")
        XCTAssertNil(chart.errorMessage,
                     "Empty result is not an error — errorMessage must stay nil")
    }

    // MARK: - Refresh (AC-7 tests 4-8)

    /// `refresh()` must invoke `ffiRefreshVendor` exactly once per known
    /// vendor (4 today: anthropic, openai, openrouter, zai). Order matters
    /// for the serial loop — assert the captured names match the canonical
    /// order from `refreshableVendors` constant.
    func test_refresh_calls_ffi_refresh_for_each_vendor() async {
        var capturedVendors: [String] = []
        let (chart, main) = makeSubject(
            refreshFn: { vendor in capturedVendors.append(vendor) }
        )
        _ = main  // retain — ChartViewModel holds mainViewModel as weak

        await waitForInflightWork()  // let the initial reload settle
        await chart.refresh()

        XCTAssertEqual(capturedVendors, ["anthropic", "openai", "openrouter", "zai"],
                       "refresh() must call ffiRefreshVendor once per vendor in canonical order")
    }

    /// `CredentialMissing` is the expected first-run state for vendors the
    /// user hasn't configured. `refresh()` must NOT surface this as an
    /// error in `refreshErrors` (AC-5 of Story 5.3). When ALL vendors miss
    /// credentials, the orientation hint (AC-6) takes over instead.
    func test_refresh_swallows_credentialMissing_silently() async {
        let (chart, main) = makeSubject(
            refreshFn: { _ in
                throw RefreshFfiError.CredentialMissing(message: "no creds")
            }
        )
        _ = main  // retain — ChartViewModel holds mainViewModel as weak

        await waitForInflightWork()
        await chart.refresh()

        // All 4 vendors returned CredentialMissing → orientation hint banner.
        XCTAssertEqual(chart.refreshStatusKind, .info,
                       "All-missing-creds path should publish the orientation hint, not an error")
        XCTAssertTrue(chart.refreshStatusMessage?.contains("Settings") == true,
                      "Hint message should reference Settings. Got: \(chart.refreshStatusMessage ?? "<nil>")")
    }

    /// Non-`CredentialMissing` failures (NetworkError, ApiError, etc.) must
    /// be aggregated into the `refreshStatusMessage` with `kind == .error`.
    /// We mix one network failure with three credential-missing vendors to
    /// assert the error path wins (priority rule from `ChartViewModel`).
    func test_refresh_aggregates_refreshErrors_for_non_credential_errors() async {
        let (chart, main) = makeSubject(
            refreshFn: { vendor in
                if vendor == "anthropic" {
                    throw RefreshFfiError.NetworkError(message: "timeout")
                }
                throw RefreshFfiError.CredentialMissing(message: "no creds")
            }
        )
        _ = main  // retain — ChartViewModel holds mainViewModel as weak

        await waitForInflightWork()
        await chart.refresh()

        XCTAssertEqual(chart.refreshStatusKind, .error,
                       "Real error must override the orientation hint")
        XCTAssertTrue(chart.refreshStatusMessage?.contains("anthropic") == true,
                      "Error banner should list the failing vendor. Got: \(chart.refreshStatusMessage ?? "<nil>")")
    }

    /// `isRefreshing` flips true at the start of `refresh()` and back to
    /// false at the end. We observe via Combine, capturing every emission
    /// of the published flag during the lifetime of the call.
    func test_refresh_sets_isRefreshing_true_during_inflight_false_after() async {
        var observedStates: [Bool] = []
        var cancellables: Set<AnyCancellable> = []
        let (chart, main) = makeSubject(
            refreshFn: { _ in
                // Yield briefly so the publisher has a chance to capture
                // the `true` state before we return.
                await Task.yield()
            }
        )
        _ = main  // retain — ChartViewModel holds mainViewModel as weak

        chart.$isRefreshing
            .sink { observedStates.append($0) }
            .store(in: &cancellables)

        await waitForInflightWork()
        await chart.refresh()
        await waitForInflightWork()

        XCTAssertTrue(observedStates.contains(true),
                      "isRefreshing must transition through `true` during the inflight window. Observed: \(observedStates)")
        XCTAssertEqual(chart.isRefreshing, false,
                       "isRefreshing must settle back to false after refresh() completes")
        _ = cancellables  // silence unused warning
    }

    /// After every refresh, the chart must re-query so the visualisation
    /// reflects the freshly-written snapshots. We assert `queryFn` is
    /// invoked at least twice: once for the initial reload at init, once
    /// for the post-refresh reload triggered by the `mainViewModel`
    /// `dateRange` re-emission inside `refresh()`.
    ///
    /// Implementation detail (why this test isn't trivial): `reload(for:)`
    /// is fire-and-forget — it creates an `inFlightTask` and returns. The
    /// task runs on the MainActor sometime after `refresh()` returns. We
    /// use `drainMainActor()` (real sleeps interleaved with yields) to
    /// give the runtime enough breathing room to schedule the new task.
    /// Empirically, 200 ms is plenty on M-series hardware; the suite still
    /// runs in well under a second.
    func test_refresh_calls_reload_after_completion() async throws {
        var queryCount = 0
        // Retain `main` explicitly: ChartViewModel holds it as `weak`, so
        // `let _ = main` would let ARC release it and `mainViewModel?` in
        // `refresh()` would short-circuit before reaching `reload(for:)`.
        let (chart, main) = makeSubject(
            refreshFn: { _ in },
            queryFn: { _, _, _, _, _, _ in
                queryCount += 1
                return []
            }
        )
        _ = main  // suppress unused-warning while preserving the strong ref

        try await pollUntil(timeout: 1.0) { queryCount >= 1 }
        let queryCountAfterInit = queryCount
        XCTAssertGreaterThanOrEqual(queryCountAfterInit, 1,
                                    "Initial reload should have fired queryFn at least once")

        await chart.refresh()
        try await drainMainActor()
        try await pollUntil(timeout: 2.0) { queryCount > queryCountAfterInit }

        XCTAssertGreaterThan(queryCount, queryCountAfterInit,
                             "refresh() must trigger an additional reload(for:) → queryFn call (queryCount was \(queryCountAfterInit), is \(queryCount))")
    }

    /// Polls `condition` every 5 ms (with a real wall-clock sleep so the
    /// MainActor queue actually drains) until `condition()` returns true
    /// or `timeout` seconds elapse.
    private func pollUntil(
        timeout: TimeInterval,
        condition: () -> Bool
    ) async throws {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if condition() { return }
            try await Task.sleep(nanoseconds: 5_000_000)  // 5 ms
        }
    }

    /// Drives all pending MainActor work to completion by alternating
    /// `Task.yield()` (queue drain) with brief real sleeps (gives the
    /// scheduler time to advance unactivated Tasks). 200 ms total is enough
    /// for any in-process FFI-stubbed work to settle without making the
    /// test suite slow.
    private func drainMainActor() async throws {
        for _ in 0..<10 {
            try await Task.sleep(nanoseconds: 20_000_000)  // 20 ms
            await Task.yield()
        }
    }
}
