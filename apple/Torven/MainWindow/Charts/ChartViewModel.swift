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

    /// Story 5.3 (Wave 5): true while `refresh()` is in flight. The Main
    /// Window top bar binds the Refresh button's `.disabled` to this so the
    /// user can't double-click and trigger overlapping serial fetches.
    @Published private(set) var isRefreshing: Bool = false

    /// Story 5.3 AC-5/AC-6: ephemeral status message rendered below the top
    /// bar after `refresh()` completes. `nil` = no banner. Populated with one
    /// of three shapes:
    ///   - "Refresh parcial: {vendors} falhou ({reason})"  (AC-5)
    ///   - "Nenhum vendor configurado. Abra Settings (⌘,) ..."  (AC-6)
    ///   - nil  (success — chart + budget already updated by reload())
    /// A trailing Task clears this back to nil after a short delay (5s for
    /// error, 8s for the "no config" hint) — see `scheduleRefreshMessageClear`.
    @Published private(set) var refreshStatusMessage: String?

    /// Tone for `refreshStatusMessage`. Drives the banner's `foregroundStyle`
    /// in `MainWindowView` so AC-5 (error: red-ish) and AC-6 (hint: subtle)
    /// render with the appropriate visual weight without the View having to
    /// regex the message string.
    @Published private(set) var refreshStatusKind: RefreshStatusKind = .info

    /// Kind discriminator for `refreshStatusMessage`. `.error` is used when
    /// at least one vendor failed with a non-`CredentialMissing` error;
    /// `.info` is used for the "no vendor configured" orientation hint.
    enum RefreshStatusKind {
        case error
        case info
    }

    /// Story 4.4 AC-3: vendor currently isolated by the user via the
    /// Per-vendor grid. `nil` = no filter (show all vendors stacked or all
    /// mini-charts un-highlighted). When non-nil, the Aggregate view
    /// (`StackedAreaChart`) restricts itself to this vendor and the
    /// Per-vendor grid draws a colored border around the matching card.
    @Published private(set) var selectedVendor: String?

    /// Story 4.6 (Wave 4): month-to-date budget burn status, fetched
    /// alongside each chart reload via `getBudgetStatus()`. Defaulted to a
    /// `hasBudget == false` value so `BudgetBurn` renders `EmptyView()`
    /// before the first reload completes (no flash of stale data).
    @Published private(set) var budgetStatus: BudgetStatus = BudgetStatus(
        totalSpentUsd: 0.0,
        totalBudgetUsd: nil,
        totalPercentUsed: 0.0,
        perVendor: [],
        hasBudget: false
    )

    /// Story 4.4 AC-3: `chartData.samples` grouped by `.vendor`. Computed
    /// on demand so we don't have to invalidate a stored cache every time
    /// `chartData` is republished. `Dictionary(grouping:by:)` is O(n) and
    /// `n` is bounded by the FFI worst case (5 vendors × 168 hourly buckets
    /// = 840 samples for 7d), so this is cheap enough to recompute on each
    /// view evaluation that needs it.
    var samplesByVendor: [String: [AggregatedSample]] {
        Dictionary(grouping: chartData.samples, by: \.vendor)
    }

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

    // Story 5.3: tracks the most-recent refresh-message clear task so a new
    // refresh that completes before the previous timer fires can cancel the
    // pending clear and reset its own deadline. Without this, two refreshes
    // 2s apart would have their banners disappear out-of-order.
    private var refreshMessageClearTask: Task<Void, Never>?

    // Story 5.3: canonical list of vendors that `ffi_refresh_vendor` knows
    // how to dispatch. Kept in sync with `vendor.rs::VendorId` enum in the
    // Rust core (anthropic, openai, openrouter, zai). Hardcoded here because
    // the FFI doesn't expose an enumeration endpoint — adding a 5th vendor
    // is a deliberate cross-stack change (UDL + this constant + Settings UI).
    private static let refreshableVendors: [String] = [
        "anthropic",
        "openai",
        "openrouter",
        "zai",
    ]

    init(mainViewModel: MainWindowViewModel) {
        self.mainViewModel = mainViewModel
        bindToDateRange(mainViewModel)
    }

    // MARK: - Vendor selection (Story 4.4)

    /// Toggle filter on the supplied vendor. Tapping the currently-selected
    /// vendor clears the filter (passes `nil` semantics). Tapping a
    /// different vendor swaps the selection. Tapping with `nil` clears
    /// unconditionally (used by tests / future explicit "clear filter"
    /// affordances).
    ///
    /// Why centralise the toggle here (instead of in the view)? The view
    /// then doesn't have to know that re-tapping should deselect, which
    /// keeps `PerVendorGrid` a pure presentation layer and `ChartViewModel`
    /// the single owner of selection state.
    func selectVendor(_ vendorId: String?) {
        guard let vendorId else {
            selectedVendor = nil
            return
        }
        if selectedVendor == vendorId {
            selectedVendor = nil
        } else {
            selectedVendor = vendorId
        }
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

                // Story 4.6: refresh the budget burn alongside the chart.
                // `getBudgetStatus()` is synchronous (no [Async] in UDL) and
                // server-side returns `hasBudget = false` gracefully when
                // SQLite is uninitialised — so we can call it here without
                // additional error handling. Doing the fetch in the same
                // Task keeps the gauge in sync with the rest of the view's
                // refresh cadence, and the call is cheap (early-returns
                // when no `[budgets]` is configured).
                self.budgetStatus = getBudgetStatus()
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

    // MARK: - Refresh (Story 5.3)

    /// Manual refresh entry point bound to the Main Window's "Refresh" button
    /// and the ⌘R keyboard shortcut. For each of the 4 known vendors, calls
    /// `ffiRefreshVendor` (Story 5.1) — vendors without credentials fail
    /// silently via `RefreshFfiError.CredentialMissing`. After all vendors
    /// finish, calls `reload(for:)` so the chart and BudgetBurn gauge reflect
    /// the freshly-fetched snapshots.
    ///
    /// Concurrency: vendors are dispatched **serially** in Wave 5 (per
    /// "Out of Scope" in the story spec — parallel via `TaskGroup` is
    /// Wave 5.5). Worst case ≈ 4 × 10s timeout = 40s, which the manual-only
    /// trigger absorbs gracefully (button stays disabled, spinner visible).
    ///
    /// AC-2: flips `isRefreshing` true → false around the entire serial loop
    /// plus the trailing `reload(for:)`. `defer` guarantees the flag clears
    /// even on early-return paths or unexpected throws.
    ///
    /// AC-5/6: aggregates non-`CredentialMissing` failures into a single
    /// banner string; if every vendor returns `CredentialMissing`, publishes
    /// the orientation hint pointing at Settings (⌘,) instead.
    ///
    /// AC-9 (Cmd+R conflict): no other component captures Cmd+R in the
    /// Wave 4 Main Window — verified by grep of `keyboardShortcut` across
    /// `apple/Torven/**.swift`; only the new Refresh button uses it.
    func refresh() async {
        // Guard: if a refresh is already running, ignore the call. The
        // button's `.disabled(isRefreshing)` makes this unreachable through
        // the UI, but Cmd+R could in theory race the first state update.
        guard !isRefreshing else { return }

        isRefreshing = true
        defer { isRefreshing = false }

        // Aggregate failure ledger. `missingCount` tracks vendors that
        // returned `CredentialMissing` so AC-6 can fire when **all** vendors
        // are unconfigured (vs AC-5 which only counts real errors).
        var errorsByVendor: [String: String] = [:]
        var missingCount = 0

        for vendor in Self.refreshableVendors {
            do {
                try await ffiRefreshVendor(vendorName: vendor)
            } catch let error as RefreshFfiError {
                switch error {
                case .CredentialMissing:
                    // Expected for vendors the user hasn't configured yet
                    // (no Keychain blob for openrouter/zai, or no OAuth
                    // credentials file for anthropic/openai). Silently
                    // increment the counter — never surfaced as an error.
                    missingCount += 1
                case .NetworkError(let message),
                     .ApiError(let message),
                     .ParseFailure(let message),
                     .StorageError(let message):
                    errorsByVendor[vendor] = message
                }
            } catch {
                // Catch-all for anything the uniffi layer might surface
                // outside `RefreshFfiError` (cancellation, future async
                // refactor edge cases). Treat as a generic failure.
                errorsByVendor[vendor] = error.localizedDescription
            }
        }

        // Publish status banner BEFORE the chart reload so the user sees
        // the outcome immediately. AC-5 vs AC-6 priority: real errors win
        // — if 3 vendors are missing creds and 1 vendor failed with a
        // NetworkError, show the network error (the user can fix Settings
        // separately).
        if !errorsByVendor.isEmpty {
            let vendorList = errorsByVendor.keys.sorted().joined(separator: ", ")
            refreshStatusKind = .error
            refreshStatusMessage = "Refresh parcial: \(vendorList) falhou."
            scheduleRefreshMessageClear(afterSeconds: 5)
        } else if missingCount == Self.refreshableVendors.count {
            // AC-6: ALL vendors returned CredentialMissing — orientation hint.
            refreshStatusKind = .info
            refreshStatusMessage = "Nenhum vendor configurado. Abra Settings (⌘,) para adicionar suas API keys."
            scheduleRefreshMessageClear(afterSeconds: 8)
        } else {
            // At least one vendor succeeded and none failed loudly. Clear
            // any stale banner from a prior refresh so the success state is
            // visually unambiguous.
            refreshMessageClearTask?.cancel()
            refreshStatusMessage = nil
        }

        // Reload the chart + budget gauge with the freshly-written snapshots.
        // We use `mainViewModel.dateRange` rather than a stored copy so the
        // refresh respects whatever range the user picked between clicks.
        if let range = mainViewModel?.dateRange {
            reload(for: range)
        }
    }

    /// Schedules a delayed clear of `refreshStatusMessage`. Cancels any
    /// pending clear so refreshes that pile up reset the deadline rather
    /// than each setting their own (which would cause out-of-order dismissal).
    private func scheduleRefreshMessageClear(afterSeconds seconds: Int) {
        refreshMessageClearTask?.cancel()
        refreshMessageClearTask = Task { @MainActor [weak self] in
            // `Task.sleep` is cancellation-aware; if the task is cancelled
            // before the deadline (e.g. a new refresh triggers a new clear)
            // the `try` throws `CancellationError` and we fall through
            // without nilling the message.
            do {
                try await Task.sleep(nanoseconds: UInt64(seconds) * 1_000_000_000)
            } catch {
                return
            }
            guard let self else { return }
            self.refreshStatusMessage = nil
        }
    }
}
