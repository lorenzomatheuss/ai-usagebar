//
//  MainWindowView.swift
//  Torven
//
//  Wave 4 Main Window root. Owns the `MainWindowViewModel` and lays out:
//    - top bar : DateRangePicker (Story 4.2)
//                 + future Cost/Requests toggle (Story 4.5)
//    - content : StackedAreaChart (Story 4.3) driven by a `ChartViewModel`
//                that observes the same `$dateRange`. Stories 4.4-4.6 will
//                stack additional charts in this area.
//

import SwiftUI

struct MainWindowView: View {
    @StateObject private var viewModel = MainWindowViewModel()

    var body: some View {
        VStack(spacing: 0) {
            topBar
                .padding(.horizontal, 16)
                .padding(.vertical, 10)

            Divider()

            // Push the chart wiring into a sub-view so its `ChartViewModel`
            // can be a `@StateObject` initialized from the surrounding
            // `MainWindowViewModel`. This is the canonical macOS-13 pattern
            // for "one view-model depends on another at init time" — a
            // `@StateObject(wrappedValue:)` initializer in this top-level
            // view can't reference `viewModel` because the property hasn't
            // been published into `self` yet.
            ChartContent(mainViewModel: viewModel)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var topBar: some View {
        HStack(spacing: 12) {
            DateRangePicker(dateRange: $viewModel.dateRange)
            Spacer()
        }
    }
}

// MARK: - Chart content sub-view

/// Hosts the chart-side view models so they can `@StateObject`-init from the
/// shared `MainWindowViewModel`. Kept private to this file because no other
/// site needs to compose chart+range — and centralizing the layout here means
/// future stories adding sibling charts (4.4 mini-charts, 4.6 budget gauge)
/// don't have to retrofit a different ownership shape.
private struct ChartContent: View {
    @ObservedObject var mainViewModel: MainWindowViewModel
    @StateObject private var chartViewModel: ChartViewModel

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
            StackedAreaChart(chartData: chartViewModel.chartData)
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
}

#Preview {
    MainWindowView()
        .frame(width: 900, height: 600)
}
