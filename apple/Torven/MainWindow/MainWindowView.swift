//
//  MainWindowView.swift
//  Torven
//
//  Wave 4 Main Window root. Owns the `MainWindowViewModel` and lays out:
//    - top bar : DateRangePicker (Story 4.2)
//                 + ViewMode segmented picker (Story 4.4)
//                 + future Cost/Requests toggle (Story 4.5)
//    - content : StackedAreaChart (Story 4.3) OR PerVendorGrid (Story 4.4)
//                driven by a `ChartViewModel` that observes the same
//                `$dateRange`. Stories 4.5-4.6 will stack additional charts
//                in this area.
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
            Picker("View", selection: $viewMode) {
                ForEach(ChartViewMode.allCases) { mode in
                    Text(mode.rawValue).tag(mode)
                }
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .frame(width: 220)
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
                    selectedVendor: chartViewModel.selectedVendor
                )
            case .perVendor:
                PerVendorGrid(
                    chartData: chartViewModel.chartData,
                    samplesByVendor: chartViewModel.samplesByVendor,
                    selectedVendor: chartViewModel.selectedVendor,
                    onVendorSelected: { vendor in
                        chartViewModel.selectVendor(vendor)
                    }
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
