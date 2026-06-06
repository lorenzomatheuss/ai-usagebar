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

    /// Story 4.5 AC-3: chosen Y-axis dimension. Default `.cost` keeps Story
    /// 4.4 callers source-compatible.
    var metric: ChartMetric = .cost

    /// Story 4.5: projects an `AggregatedSample` onto the current metric.
    /// Returns `Double` for chart-mark compatibility even though
    /// `requestCount` is integral.
    private func metricValue(_ sample: AggregatedSample) -> Double {
        switch metric {
        case .cost: return sample.costUsd
        case .requests: return Double(sample.requestCount)
        }
    }

    /// Total of the period for this vendor (replaces 4.4's `totalCost`).
    /// Renamed to `totalValue` because, post-4.5, the unit is metric-
    /// dependent — "cost" in the name would mislead under `.requests`.
    /// Still `Double` so the existing `.currency` formatter path keeps
    /// working without a generic dance.
    private var totalValue: Double {
        samples.reduce(0) { $0 + metricValue($1) }
    }

    /// Story 4.5: user-facing total string. Mirrors the formatting choice in
    /// `StackedAreaChart.formattedMetricValue(for:)` so both the per-vendor
    /// card header and the aggregate hover card speak the same language.
    private var formattedTotal: String {
        switch metric {
        case .cost:
            return "\(totalValue.formatted(.currency(code: "USD"))) total"
        case .requests:
            // `totalValue` was summed as `Double` to stay AreaMark-friendly,
            // but a request count is logically an integer — convert back at
            // the display boundary so we don't render "10.0 requests".
            return "\(Int(totalValue.rounded())) requests"
        }
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
        // Story 4.5: VoiceOver label tracks the active metric so screen-
        // reader users get the same Cost ↔ Requests context that sighted
        // users get from the header.
        .accessibilityLabel("\(displayName) \(metric.rawValue.lowercased()) chart, total \(formattedTotal)")
    }

    // MARK: - Header

    private var header: some View {
        HStack(alignment: .firstTextBaseline) {
            VStack(alignment: .leading, spacing: 2) {
                Text(displayName)
                    .font(.headline)
                    .foregroundStyle(.primary)
                // Story 4.5: subtitle string flips with `metric`. Hits the
                // single `formattedTotal` computed property so the header,
                // accessibility label, and any future tooltip stay in sync.
                Text(formattedTotal)
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
                        y: .value(metric.yAxisLabel, metricValue(sample))
                    )
                    .foregroundStyle(vendorColor)
                    .interpolationMethod(.catmullRom)
                }
            }
            // Compact axes — the 5-up grid means each card is small; full
            // currency-formatted ticks would visually crowd. Auto y-axis
            // and an aligned x-axis is enough at this scale. Story 4.5 keeps
            // the y-axis `.automatic` regardless of metric: the small-card
            // ticks are unitless visual scale cues; the header already names
            // the unit ("USD total" vs "N requests"), so formatting the
            // ticks differently would just be noise at this density.
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

#Preview("Populated · Cost") {
    MiniVendorChart(
        vendorId: "anthropic",
        samples: (0..<24).map { hour in
            AggregatedSample(
                timestamp: Calendar.current.date(byAdding: .hour, value: hour, to: Date().addingTimeInterval(-86400)) ?? Date(),
                vendor: "anthropic",
                costUsd: 0.10 + 0.02 * sin(Double(hour) / 24.0 * 2 * .pi),
                requestCount: 12
            )
        },
        metric: .cost
    )
    .padding()
    .frame(width: 300, height: 200)
}

#Preview("Populated · Requests") {
    MiniVendorChart(
        vendorId: "anthropic",
        samples: (0..<24).map { hour in
            AggregatedSample(
                timestamp: Calendar.current.date(byAdding: .hour, value: hour, to: Date().addingTimeInterval(-86400)) ?? Date(),
                vendor: "anthropic",
                costUsd: 0.10 + 0.02 * sin(Double(hour) / 24.0 * 2 * .pi),
                requestCount: 8 + Int((Double(hour) / 24.0 * 12).rounded())
            )
        },
        metric: .requests
    )
    .padding()
    .frame(width: 300, height: 200)
}

#Preview("Empty") {
    MiniVendorChart(vendorId: "openai", samples: [])
        .padding()
        .frame(width: 300, height: 200)
}
