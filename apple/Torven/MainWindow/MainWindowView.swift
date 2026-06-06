//
//  MainWindowView.swift
//  Torven
//
//  Wave 4 Main Window root. Owns the `MainWindowViewModel` and lays out:
//    - top bar : DateRangePicker (Story 4.2)
//                 + ViewMode segmented picker (Story 4.4)
//                 + ChartMetric Cost/Requests picker (Story 4.5)
//    - content : StackedAreaChart (Story 4.3) OR PerVendorGrid (Story 4.4),
//                each parametrized by ChartMetric (Story 4.5). Driven by a
//                `ChartViewModel` that observes the same `$dateRange`.
//                Story 4.6 will stack additional charts in this area.
//

import SwiftUI

// MARK: - ViewMode (Story 4.4)

/// Top-level chart view selector. Drives whether the Main Window's content
/// area renders the aggregate `StackedAreaChart` (Story 4.3) or the
/// `PerVendorGrid` of mini-charts (Story 4.4). Lifted out of `ChartContent`
/// to file scope so future stories adding modes (e.g. AR-8 alerts panel) can
/// reuse the same enum without an additional indirection.
enum ChartViewMode: String, CaseIterable, Identifiable {
    case aggregate = "Aggregate"
    case perVendor = "Per-vendor"

    var id: String { rawValue }
}

struct MainWindowView: View {
    @StateObject private var viewModel = MainWindowViewModel()

    var body: some View {
        VStack(spacing: 0) {
            // ChartContent owns the ViewMode picker so the picker and the
            // chart view model live next to each other — toggling the mode
            // is local UI state that has no business polluting the top-level
            // `MainWindowView`.
            ChartContent(mainViewModel: viewModel)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

// MARK: - Chart content sub-view

/// Hosts the chart-side view models so they can `@StateObject`-init from the
/// shared `MainWindowViewModel`. Kept private to this file because no other
/// site needs to compose chart+range — and centralizing the layout here means
/// future stories adding sibling charts (4.5 tokens chart, 4.6 budget gauge)
/// don't have to retrofit a different ownership shape.
private struct ChartContent: View {
    @ObservedObject var mainViewModel: MainWindowViewModel
    @StateObject private var chartViewModel: ChartViewModel

    /// Story 4.4 AC-4: top-level mode selector. Local state because we
    /// don't (yet) need to persist the user's last-selected mode across
    /// sessions — when 1.5+ adds settings sync this can graduate to a
    /// `@AppStorage` or a published property on `MainWindowViewModel`.
    @State private var viewMode: ChartViewMode = .aggregate

    /// Story 4.5 AC-4: chart metric (Cost vs Requests). Default `.cost` so
    /// the window opens looking identical to the post-4.4 state. Lives
    /// alongside `viewMode` as local UI state — AC-5 requires that toggling
    /// it leaves the date range and view mode untouched, which falls out
    /// naturally from the three states being independent `@State`/published
    /// properties.
    @State private var chartMetric: ChartMetric = .cost

    /// AC-7: respect Reduce Motion. The implicit fade between modes (driven
    /// by `.animation(_, value: viewMode)` below) becomes `nil` when the
    /// system setting is on, so the swap is instant.
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    init(mainViewModel: MainWindowViewModel) {
        self.mainViewModel = mainViewModel
        // `StateObject(wrappedValue:)` is the supported escape hatch for
        // injecting an init-time dependency. It's invoked at most once per
        // view identity, so the inner ChartViewModel survives re-renders
        // just like a parameter-less `@StateObject`.
        _chartViewModel = StateObject(
            wrappedValue: ChartViewModel(mainViewModel: mainViewModel)
        )
    }

    var body: some View {
        VStack(spacing: 0) {
            topBar
                .padding(.horizontal, 16)
                .padding(.vertical, 10)

            Divider()

            // Story 5.3 (AC-5/AC-6): refresh status banner. Rendered above
            // the chart so a partial-failure or "no vendor configured" hint
            // doesn't obscure the chart contents. Distinct from the
            // `errorMessage` band below the chart — that one surfaces
            // `ffi_query_aggregated` failures (Wave 4), this one surfaces
            // `ffi_refresh_vendor` failures (Wave 5).
            if let statusMessage = chartViewModel.refreshStatusMessage {
                Text(statusMessage)
                    .font(.caption)
                    .foregroundStyle(
                        chartViewModel.refreshStatusKind == .error
                            ? Color.red
                            : Color.secondary
                    )
                    .padding(.horizontal, 16)
                    .padding(.vertical, 6)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(
                        chartViewModel.refreshStatusKind == .error
                            ? Color.red.opacity(0.08)
                            : Color.secondary.opacity(0.05)
                    )
                    .accessibilityLabel(statusMessage)
            }

            chartBody

            if let errorMessage = chartViewModel.errorMessage {
                // Lightweight inline error band. Empty state already shows
                // when chartData.isEmpty; this banner gives the user a
                // breadcrumb of *why* the chart is empty when the cause is
                // an FFI failure (vs simply no data).
                Text(errorMessage)
                    .font(.caption)
                    .foregroundStyle(.red)
                    .padding(.horizontal, 16)
                    .padding(.vertical, 6)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(Color.red.opacity(0.08))
            }

            // Story 4.6 (AC-6): budget burn footer. `BudgetBurn` itself
            // returns `EmptyView()` when no `[budgets]` is configured, but
            // we ALSO gate the leading `Divider()` on `hasBudget` so the
            // footer collapses to zero pixels for users without a budget
            // (no orphan divider line at the window's bottom edge).
            if chartViewModel.budgetStatus.hasBudget {
                Divider()
                BudgetBurn(status: chartViewModel.budgetStatus)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // MARK: - Top bar

    private var topBar: some View {
        HStack(spacing: 12) {
            DateRangePicker(dateRange: $mainViewModel.dateRange)

            Spacer()

            // Story 4.4 AC-4: segmented Aggregate / Per-vendor switch.
            // `.labelsHidden()` because the segmented control's segments
            // are self-describing — the leading "View" label would just
            // add clutter at this width.
            //
            // Story 4.5 layout note (top-bar density): the default 900pt
            // window now hosts three controls + the date picker. To stay
            // on a single line even when DateRangePicker expands into
            // Custom mode (two DatePickers ≈ +280pt), both segmented
            // pickers were tightened — ViewMode 220→180, ChartMetric set
            // to 160. Resulting estimate at 900pt: ~280 (date) + 180 (view)
            // + 160 (metric) + 36 (paddings/spacing) = ~656pt, leaving
            // ~244pt of headroom for the custom-range two-DatePicker case.
            Picker("View", selection: $viewMode) {
                ForEach(ChartViewMode.allCases) { mode in
                    Text(mode.rawValue).tag(mode)
                }
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .frame(width: 180)

            // Story 4.5 AC-4: segmented Cost / Requests metric switch.
            // Distinct from the ViewMode picker conceptually — view mode is
            // "how do I look at this data?", metric is "which dimension?".
            // Both are segmented for parity with the existing 4.4 control
            // pattern.
            Picker("Metric", selection: $chartMetric) {
                ForEach(ChartMetric.allCases) { metric in
                    Text(metric.rawValue).tag(metric)
                }
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .frame(width: 160)

            // Story 5.3 (AC-1, AC-2, AC-7): manual refresh trigger. Icon-only
            // to preserve top-bar density (existing controls already consume
            // ~656pt at 900pt window; adding "Refresh" text would push the
            // bar past the safe limit when the date picker is in Custom mode
            // with two DatePickers). The `accessibilityLabel` carries the
            // human-readable description for VoiceOver users.
            //
            // ⌘R is the canonical "refresh" shortcut on macOS (Safari, Mail,
            // Finder) — verified by grep that no other component in the
            // Main Window claims it (AC-9: no conflict).
            //
            // `.borderless` keeps the button visually quiet next to the
            // segmented pickers; `.disabled(isRefreshing)` blocks double-
            // clicks while a refresh is in flight.
            Button {
                Task { await chartViewModel.refresh() }
            } label: {
                if chartViewModel.isRefreshing {
                    // While refreshing: small inline ProgressView in place
                    // of the arrow icon. `.controlSize(.small)` matches the
                    // visual weight of the surrounding controls.
                    ProgressView()
                        .controlSize(.small)
                        .frame(width: 16, height: 16)
                } else {
                    Image(systemName: "arrow.clockwise")
                        .frame(width: 16, height: 16)
                }
            }
            .buttonStyle(.borderless)
            .disabled(chartViewModel.isRefreshing)
            .keyboardShortcut("r", modifiers: .command)
            .accessibilityLabel("Atualizar dados de uso")
            .accessibilityHint("Busca dados reais das APIs configuradas")
        }
    }

    // MARK: - Chart body

    @ViewBuilder
    private var chartBody: some View {
        Group {
            switch viewMode {
            case .aggregate:
                StackedAreaChart(
                    chartData: chartViewModel.chartData,
                    selectedVendor: chartViewModel.selectedVendor,
                    metric: chartMetric
                )
            case .perVendor:
                PerVendorGrid(
                    chartData: chartViewModel.chartData,
                    samplesByVendor: chartViewModel.samplesByVendor,
                    selectedVendor: chartViewModel.selectedVendor,
                    onVendorSelected: { vendor in
                        chartViewModel.selectVendor(vendor)
                    },
                    metric: chartMetric
                )
            }
        }
        // AC-7: skip the implicit cross-fade when Reduce Motion is on. We
        // animate on `viewMode` so the transition only fires for the user-
        // driven mode switch, not for every `chartData` republish.
        .animation(reduceMotion ? nil : .easeInOut(duration: 0.2),
                   value: viewMode)
    }
}

#Preview {
    MainWindowView()
        .frame(width: 900, height: 600)
}
