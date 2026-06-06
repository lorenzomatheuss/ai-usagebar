//
//  ChartViewModel.swift
//  Torven
//
//  Story 4.3 (Wave 4): bridges `MainWindowViewModel.$dateRange` (Story 4.2)
//  to the SQLite aggregation pipeline exposed by `ffi_query_aggregated`
//  (Story 4.0.5), publishing the result as a `ChartData` value that
//  `StackedAreaChart` (and, later, Stories 4.4/4.5/4.6) render.
//
//  ## Why a dedicated `ChartViewModel` instead of folding this into
//  `MainWindowViewModel`?
//
//  Story 4.2's MainWindowViewModel was deliberately scoped as a "pure state
//  holder" with no FFI dependency, so it could ship before the 4.0.5
//  aggregation query existed. Splitting the chart-FFI concern out:
//    - keeps the date-range picker (4.2) unit-testable without an FFI mock,
//    - lets Stories 4.4-4.6 instantiate sibling chart view models that share
//      the same `dateRange` source without crowding a single god-object,
//    - matches the ADR-10 layering: SwiftUI view models own their own
//      async/error state.
//
//  ## Combine over `.onChange`
//
//  Risk #3 in the story doc: `View.onChange` has two different signatures
//  pre/post macOS 14. `.onReceive(mainViewModel.$dateRange)` works
//  unchanged on macOS 13.0 (our deployment target) and 14+.
//
//  ## Lazy `ffi_init_history`
//
//  `ffi_query_aggregated` requires a prior `ffi_init_history` call (writes
//  the per-process `HistoryDb` slot). Wave 4 has not added an app-startup
//  init hook yet; calling it lazily on first reload is idempotent on the
//  Rust side (`*slot = Some(db)` overwrites unconditionally — see
//  `crates/torven-core/src/uniffi_exports.rs#ffi_init_history`). Failing
//  this call surfaces as a `HistoryFfiError.Storage` → published as an
//  error string; the chart degrades to the empty-state view (AC-4).
//

import Combine
import Foundation
import SwiftUI

@MainActor
final class ChartViewModel: ObservableObject {
    /// Latest result of `ffi_query_aggregated` for `mainViewModel.dateRange`.
    /// Set to `.empty` on init so the view's empty-state path renders
    /// without an explicit "loading" flash on first appearance.
    @Published private(set) var chartData: ChartData = .empty

    /// Non-nil when the last `reload(for:)` failed. Surfaced to the view
    /// alongside the empty-state path (AC-4) — we keep the view simple by
    /// folding loading-failures into the empty visual, with the error text
    /// shown below the icon so the user knows it's not "no data" but "couldn't
    /// load".
    @Published private(set) var errorMessage: String?

    private var cancellables = Set<AnyCancellable>()
    private weak var mainViewModel: MainWindowViewModel?

    // Tracks the most-recent reload so range changes that arrive while a
    // prior fetch is still in flight cancel the stale work and only the
    // latest range's result is published. Without this, fast 7d → 30d →
    // Custom switching could land an older range's data on top of a newer
    // one's because async FFI returns aren't ordered.
    private var inFlightTask: Task<Void, Never>?

    // Lazy idempotent flag so we only attempt `ffi_init_history` once per
    // ViewModel lifetime. If init fails the error path publishes a message
    // and the next `reload(for:)` will retry init before querying.
    private var historyInitialized = false

    init(mainViewModel: MainWindowViewModel) {
        self.mainViewModel = mainViewModel
        bindToDateRange(mainViewModel)
    }

    // MARK: - Wiring

    /// Subscribes to `mainViewModel.$dateRange`. Each emission triggers a
    /// fresh `reload(for:)` — the initial published value is delivered
    /// immediately by Combine, so we don't need a separate "first load"
    /// kick.
    private func bindToDateRange(_ vm: MainWindowViewModel) {
        vm.$dateRange
            // `removeDuplicates` guards against extra emissions if a future
            // refactor re-publishes the same range (e.g. on app-foreground
            // refresh). Cheap insurance.
            .removeDuplicates()
            .sink { [weak self] range in
                self?.reload(for: range)
            }
            .store(in: &cancellables)
    }

    // MARK: - FFI reload

    /// Public entry point: re-fetch the aggregated buckets for `range` and
    /// publish them as `chartData`. Exposed (rather than private) so tests
    /// and Stories 4.4-4.6 sibling view models can drive a reload without
    /// going through Combine.
    func reload(for range: DateRange) {
        // Cancel any in-flight work — only the newest range's result should
        // win. `Task.isCancelled` is checked inside the body before
        // publishing, so a cancelled fetch is dropped silently.
        inFlightTask?.cancel()

        inFlightTask = Task { @MainActor [weak self] in
            guard let self else { return }

            // Lazy history init. Idempotent on the Rust side, so re-calling
            // on every retry-after-failure is safe.
            if !self.historyInitialized {
                do {
                    try ffiInitHistory(dbPath: nil)
                    self.historyInitialized = true
                } catch {
                    self.errorMessage = "Failed to open history database: \(error.localizedDescription)"
                    self.chartData = .empty
                    return
                }
            }

            // The FFI call itself is synchronous from Swift's perspective
            // (uniffi-generated free function), but we're inside a Task so
            // the synchronous SQLite work doesn't block the UI thread *if*
            // the runtime decides to suspend us. In practice for Wave 4's
            // expected payload (≤ 840 buckets) this is fast (< 50ms) and
            // the @MainActor isolation is a fine trade-off; Story 4.4+ can
            // move the FFI off-main if profiling shows it matters.
            do {
                let buckets = try ffiQueryAggregated(
                    vendor: "",
                    accountFilterMode: .all,
                    accountId: nil,
                    sinceTs: range.sinceTs,
                    untilTs: range.untilTs,
                    bucketStrategy: .auto
                )

                // Guard: if the task was cancelled while the FFI was
                // running (e.g. user clicked a different range), drop the
                // result so we don't clobber a fresher publish.
                guard !Task.isCancelled else { return }

                self.chartData = AggregatedSample.fromFFI(buckets)
                self.errorMessage = nil
            } catch let error as HistoryFfiError {
                // Re-running init may help if the slot was lost (shouldn't
                // happen mid-process but cheap defence against a future
                // refactor that resets the slot).
                self.historyInitialized = false
                self.errorMessage = "Failed to load history: \(error.localizedDescription)"
                self.chartData = .empty
            } catch {
                // Catch-all for anything the uniffi layer might surface
                // outside `HistoryFfiError` (e.g. a `CancellationError`
                // bubbling up from a future async refactor).
                self.errorMessage = "Failed to load history: \(error.localizedDescription)"
                self.chartData = .empty
            }
        }
    }
}
