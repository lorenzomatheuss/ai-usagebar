//
//  MiniVendorChart.swift
//  Torven
//
//  Story 4.4 (Wave 4) — one card in `PerVendorGrid`: a single vendor's line
//  chart with a header (display name + total cost in period) and an inline
//  empty state. Tap propagates the vendor id up to the grid via `onTap`, so
//  the parent `ChartViewModel.selectVendor(_:)` can toggle the filter.
//
//  ## Why `LineMark` (not `AreaMark`)?
//
//  The stacked area chart (Story 4.3) shows cumulative contribution per
//  vendor — `AreaMark`s implicitly stack and shade. A single-vendor mini
//  chart has no stacking to express; `LineMark` reads as "this vendor's
//  cost-over-time" without the visual weight of a filled area, which keeps
//  the 5-up grid scanable. `.catmullRom` interpolation mirrors the parent
//  chart for visual continuity when switching modes.
//
//  ## Why pass `samples` already filtered (instead of `ChartData` + vendorId)?
//
//  Risk #2 in the story doc: 5 mini-charts × hover handlers can add up. By
//  passing pre-filtered `[AggregatedSample]` we keep the view's data surface
//  minimal — no per-frame filter inside `body`, and the grid does the
//  grouping once via `ChartViewModel.samplesByVendor`.
//
//  ## Why a custom display name helper (not a `Vendor` struct field)?
//
//  Risk #1: a `VendorInfo` lookup would force a `TorvenCoreBridge` dependency
//  into the chart layer. The shared `chartVendorDisplayName(_:)` in
//  `AggregatedSample.swift` is a small switch — cheap, dependency-free, and
//  the compiler can flag missing cases when a new vendor is added.
//
//  ## macOS 13 compatibility
//
//  - `.background { ... }` content-builder closure exists on macOS 13.
//  - Chart annotation via `.chartOverlay` (not `.annotation(position:)`) is
//    chosen so we can match the Story 4.3 hover pattern at smaller scale.
//

import Charts
import SwiftUI

struct MiniVendorChart: View {
    let vendorId: String
    let samples: [AggregatedSample]
    /// Optional tap callback — `PerVendorGrid` wires this to
    /// `ChartViewModel.selectVendor(_:)` for the click-to-filter behaviour
    /// (AC-5). `nil` makes the card non-interactive (used by previews).
    var onTap: ((String) -> Void)? = nil

    /// Total cost of the period for this vendor — header subtitle. Computed
    /// once per render; cheap (≤ 168 doubles for the worst-case 7d-hourly
    /// bucket count in `samples` for this vendor).
    private var totalCost: Double {
        samples.reduce(0) { $0 + $1.costUsd }
    }

    private var displayName: String {
        chartVendorDisplayName(vendorId)
    }

    private var vendorColor: Color {
        chartVendorColor(for: vendorId)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            header
            chartArea
        }
        .padding(12)
        .frame(minHeight: 160)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .fill(Color(nsColor: .controlBackgroundColor))
                .shadow(color: Color.black.opacity(0.08), radius: 2, x: 0, y: 1)
        )
        // The whole card is the tap target — clicking anywhere on the header
        // OR the chart area triggers selection. `.contentShape(Rectangle())`
        // ensures empty regions inside the card still register hits.
        .contentShape(Rectangle())
        .onTapGesture {
            onTap?(vendorId)
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(displayName) cost chart, total \(totalCost.formatted(.currency(code: "USD")))")
    }

    // MARK: - Header

    private var header: some View {
        HStack(alignment: .firstTextBaseline) {
            VStack(alignment: .leading, spacing: 2) {
                Text(displayName)
                    .font(.headline)
                    .foregroundStyle(.primary)
                Text("\(totalCost.formatted(.currency(code: "USD"))) total")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Spacer()
            // Small color swatch — confirms the line color matches the
            // global legend without forcing the user to read a per-card
            // legend.
            Circle()
                .fill(vendorColor)
                .frame(width: 10, height: 10)
        }
    }

    // MARK: - Chart area

    @ViewBuilder
    private var chartArea: some View {
        if samples.isEmpty {
            // Inline empty state per AC-2. Centered in the chart's frame so
            // the card height stays consistent across vendors (no layout
            // jump when one vendor has data and another doesn't).
            Text("No data")
                .font(.caption)
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
            Chart {
                ForEach(samples) { sample in
                    LineMark(
                        x: .value("Time", sample.timestamp),
                        y: .value("Cost", sample.costUsd)
                    )
                    .foregroundStyle(vendorColor)
                    .interpolationMethod(.catmullRom)
                }
            }
            // Compact axes — the 5-up grid means each card is small; full
            // currency-formatted ticks would visually crowd. Auto y-axis
            // and an aligned x-axis is enough at this scale.
            .chartXAxis {
                AxisMarks(preset: .aligned, values: .automatic(desiredCount: 3))
            }
            .chartYAxis {
                AxisMarks(values: .automatic(desiredCount: 3))
            }
            .frame(minHeight: 100)
        }
    }
}

// MARK: - Preview

#Preview("Populated") {
    MiniVendorChart(
        vendorId: "anthropic",
        samples: (0..<24).map { hour in
            AggregatedSample(
                timestamp: Calendar.current.date(byAdding: .hour, value: hour, to: Date().addingTimeInterval(-86400)) ?? Date(),
                vendor: "anthropic",
                costUsd: 0.10 + 0.02 * sin(Double(hour) / 24.0 * 2 * .pi),
                requestCount: 12
            )
        }
    )
    .padding()
    .frame(width: 300, height: 200)
}

#Preview("Empty") {
    MiniVendorChart(vendorId: "openai", samples: [])
        .padding()
        .frame(width: 300, height: 200)
}
