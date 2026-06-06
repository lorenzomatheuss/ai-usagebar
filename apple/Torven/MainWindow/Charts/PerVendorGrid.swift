//
//  PerVendorGrid.swift
//  Torven
//
//  Story 4.4 (Wave 4) — alternative to `StackedAreaChart` that shows each
//  vendor as its own `MiniVendorChart` card in an adaptive grid. Tapping a
//  card propagates the vendor id back to the parent (typically
//  `ChartViewModel.selectVendor(_:)`) so the selection survives a switch
//  back to the Aggregate view.
//
//  ## Why `LazyVGrid` with `.adaptive(minimum: 260)`?
//
//  Story risk #4 noted the 5-vendor layout could feel unbalanced (3+2 vs
//  2+2+1). Letting SwiftUI pick the column count from the available width
//  yields:
//    - small Main Window (< ~560pt usable): 2 columns → 2+2+1
//    - medium (~560-820pt): 2 columns → 2+2+1 (or 3 cols when wider)
//    - wide (> ~820pt): 3 columns → 3+2
//  `.adaptive(minimum: 260)` enforces a sensible minimum card width so a
//  user squishing the window doesn't end up with chartlets too small to
//  read. `LazyVGrid` (vs `VGrid`) is a small win — cards off-screen during
//  initial layout don't render their `Chart` content until scrolled into
//  view, which keeps the first-paint cost flat as more vendors are
//  on-boarded post-v1.0.
//
//  ## Why pass `samplesByVendor` from the parent (instead of grouping here)?
//
//  `ChartViewModel.samplesByVendor` is a computed property that runs every
//  time `chartData` changes. Computing it here would either (a) repeat the
//  work inside the view (re-grouping on every render) or (b) require a
//  `@State` cache with manual invalidation. Pushing the responsibility up
//  to the view model is cleaner and keeps this view's data dependencies
//  shallow.
//
//  ## Selection highlight (AC-5)
//
//  When `selectedVendor == vendor`, the card gets a colored border (the
//  vendor's own palette color, lineWidth 2) so the user sees which filter
//  is active. Clicking a different vendor moves the highlight; clicking
//  the already-selected vendor clears it (toggle behaviour lives in
//  `ChartViewModel.selectVendor(_:)`).
//

import SwiftUI

struct PerVendorGrid: View {
    let chartData: ChartData
    let samplesByVendor: [String: [AggregatedSample]]
    let selectedVendor: String?
    var onVendorSelected: ((String?) -> Void)? = nil

    private let columns: [GridItem] = [
        GridItem(.adaptive(minimum: 260), spacing: 16, alignment: .top)
    ]

    var body: some View {
        Group {
            if chartData.vendors.isEmpty {
                // No vendors at all — the parent already surfaces the
                // overall empty state via `StackedAreaChart`, but the
                // Per-vendor mode needs its own fallback for the case
                // where the user switches modes against an empty range.
                emptyState
            } else {
                gridContent
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var gridContent: some View {
        ScrollView {
            LazyVGrid(columns: columns, spacing: 16) {
                ForEach(chartData.vendors, id: \.self) { vendor in
                    MiniVendorChart(
                        vendorId: vendor,
                        samples: samplesByVendor[vendor] ?? [],
                        onTap: { tapped in
                            onVendorSelected?(tapped)
                        }
                    )
                    // AC-5: selection highlight. The overlay border is
                    // always drawn but with `lineWidth 0` when not
                    // selected, so the card frame stays geometrically
                    // identical on selection toggle (no jitter from a
                    // conditional `.overlay` appearing/disappearing).
                    .overlay(
                        RoundedRectangle(cornerRadius: 8)
                            .stroke(
                                chartVendorColor(for: vendor),
                                lineWidth: selectedVendor == vendor ? 2 : 0
                            )
                    )
                }
            }
            .padding(16)
        }
    }

    @ViewBuilder
    private var emptyState: some View {
        if #available(macOS 14.0, *) {
            ContentUnavailableView(
                "No Data",
                systemImage: "chart.bar.xaxis",
                description: Text("No usage history in the selected range")
            )
        } else {
            VStack(spacing: 8) {
                Image(systemName: "chart.bar.xaxis")
                    .font(.system(size: 48))
                    .foregroundStyle(.secondary)
                Text("No Data")
                    .font(.headline)
                Text("No usage history in the selected range")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }
}

// MARK: - Preview (AC-9)

#Preview("5 vendors × 7 days (mock)") {
    let mockData = PerVendorGridPreviewData.mock5x7
    return PerVendorGrid(
        chartData: mockData.chartData,
        samplesByVendor: mockData.samplesByVendor,
        selectedVendor: nil,
        onVendorSelected: nil
    )
    .frame(width: 900, height: 500)
}

#Preview("With selection") {
    let mockData = PerVendorGridPreviewData.mock5x7
    return PerVendorGrid(
        chartData: mockData.chartData,
        samplesByVendor: mockData.samplesByVendor,
        selectedVendor: "anthropic",
        onVendorSelected: nil
    )
    .frame(width: 900, height: 500)
}

#Preview("Empty") {
    PerVendorGrid(
        chartData: .empty,
        samplesByVendor: [:],
        selectedVendor: nil,
        onVendorSelected: nil
    )
    .frame(width: 900, height: 500)
}

// MARK: - Preview mock data

private enum PerVendorGridPreviewData {
    struct Mock {
        let chartData: ChartData
        let samplesByVendor: [String: [AggregatedSample]]
    }

    static var mock5x7: Mock {
        let vendors = ["anthropic", "gemini", "openai", "openrouter", "zai"]
        let calendar = Calendar.current
        let now = Date()
        let dayStart = calendar.startOfDay(for: now)

        var samples: [AggregatedSample] = []
        for dayOffset in 0..<7 {
            guard let day = calendar.date(byAdding: .day, value: -dayOffset, to: dayStart) else {
                continue
            }
            for (idx, vendor) in vendors.enumerated() {
                let base = 0.50 * Double(idx + 1)
                let dayMod = sin(Double(dayOffset) / 7.0 * 2 * .pi) * 0.30 + 0.30
                samples.append(AggregatedSample(
                    timestamp: day,
                    vendor: vendor,
                    costUsd: base + dayMod,
                    requestCount: 25 + idx * 4
                ))
            }
        }
        let chartData = ChartData(samples: samples, vendors: vendors)
        let grouped = Dictionary(grouping: samples, by: \.vendor)
        return Mock(chartData: chartData, samplesByVendor: grouped)
    }
}
