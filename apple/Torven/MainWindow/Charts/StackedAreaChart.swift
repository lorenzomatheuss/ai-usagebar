//
//  StackedAreaChart.swift
//  Torven
//
//  Story 4.3 (Wave 4): the primary chart of the Main Window — a stacked-area
//  cost-by-vendor visualization driven by `ChartData` (see
//  `AggregatedSample.swift`).
//
//  ## Why `AreaMark` + `foregroundStyle(by:)` (not `BarMark` or `LineMark`)?
//
//  ADR-6 cravado: stacked area communicates "cumulative cost per vendor
//  over time" faster than a multi-line chart (which forces the eye to
//  visually sum overlapping curves) and avoids the jagged appearance of
//  hourly bars over a 30d range. Swift Charts implicitly stacks
//  `AreaMark`s that share an x-value and differ on `foregroundStyle(by:)`
//  — no `.stacked` modifier needed.
//
//  ## Catmull-Rom interpolation
//
//  Default linear interpolation produces visible kinks at every bucket
//  boundary, which reads as noise rather than data. `.catmullRom` smooths
//  the transitions without over-smoothing (it's a true interpolation that
//  passes through every data point, unlike `.cardinal` which can wander).
//
//  ## Vendor colors (5-vendor palette)
//
//  Brand-aligned where each vendor has a distinctive marketing color, with
//  a "gemini" fallback (Google's brand blue — Wave 4 doesn't include Gemini
//  data, but the slot is reserved so future onboarding doesn't force a
//  re-color pass). Picked from `KeyValuePairs<String, Color>` rather than
//  a dictionary so the iteration order is deterministic (Swift Charts uses
//  this order for both the foreground style scale and the legend).
//
//  ## macOS 13 compatibility
//
//  - `ContentUnavailableView` is macOS 14+, so AC-4 uses `if #available`
//    with a hand-rolled VStack for 13.
//  - `onContinuousHover` exists on macOS 13.0; the hover overlay (AC-6) is
//    cross-compatible.
//  - `accessibilityReduceMotion` is on macOS 13.0; AC-7 uses the value
//    directly in the `.animation` modifier.
//
//  ## Story 4.4 cross-story modification (AC-5)
//
//  The optional `selectedVendor: String?` parameter (defaults to `nil`) was
//  added by Story 4.4 to support the "click mini-chart in Per-vendor grid
//  → filter the Aggregate stacked area to that vendor only" interaction.
//  Default `nil` keeps Story 4.3 callers source-compatible. When non-nil,
//  only that vendor's samples are rendered. The vendor color palette was
//  also promoted from a `private` file-level constant here to
//  `AggregatedSample.swift` as the shared `chartVendorColorMapping` so the
//  new `MiniVendorChart` can reuse it without duplicating the declaration.
//

import Charts
import SwiftUI

// MARK: - StackedAreaChart

struct StackedAreaChart: View {
    let chartData: ChartData

    /// Story 4.4 AC-5: when non-nil, restricts the stacked area to a single
    /// vendor so the Aggregate view reflects the selection driven by
    /// `PerVendorGrid`. Default `nil` = render every vendor (Story 4.3
    /// behaviour).
    var selectedVendor: String? = nil

    // AC-7: respect Reduce Motion — disables the implicit cross-fade Swift
    // Charts applies when the underlying data identity changes (e.g. range
    // switch). `nil` animation = instant transition.
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    // Hover state for AC-6 annotation. `hoveredSample` carries the closest
    // (in time) sample under the cursor; aggregated across vendors for the
    // current x-position via `samplesAtHoveredTimestamp`.
    @State private var hoveredTimestamp: Date?

    var body: some View {
        Group {
            if visibleSamples.isEmpty {
                emptyState
            } else {
                chartView
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    /// Samples actually drawn after applying the optional `selectedVendor`
    /// filter (Story 4.4 AC-5). When `selectedVendor` is `nil` this is the
    /// full sample set — the Story 4.3 behaviour.
    private var visibleSamples: [AggregatedSample] {
        guard let selectedVendor else { return chartData.samples }
        return chartData.samples.filter { $0.vendor == selectedVendor }
    }

    // MARK: - Chart

    private var chartView: some View {
        Chart {
            ForEach(visibleSamples) { sample in
                AreaMark(
                    x: .value("Time", sample.timestamp),
                    y: .value("Cost (USD)", sample.costUsd)
                )
                .foregroundStyle(by: .value("Vendor", sample.vendor))
                .interpolationMethod(.catmullRom)
            }

            // Visual cue for the hovered x-position. RuleMark is the
            // idiomatic way to draw a vertical guide line in Swift Charts;
            // setting it inside the same `Chart` block keeps it aligned
            // with the AreaMarks' x-scale automatically.
            if let hoveredTimestamp {
                RuleMark(x: .value("Time", hoveredTimestamp))
                    .foregroundStyle(Color.secondary.opacity(0.4))
                    .lineStyle(StrokeStyle(lineWidth: 1, dash: [4, 3]))
            }
        }
        .chartForegroundStyleScale(chartVendorColorMapping)
        .chartXAxis {
            AxisMarks(preset: .aligned)
        }
        .chartYAxis {
            AxisMarks(format: .currency(code: "USD"))
        }
        .chartLegend(position: .bottom, alignment: .center)
        // AC-6 hover overlay. `chartOverlay` gives us a `ChartProxy` that
        // can translate from view coordinates back to data values.
        .chartOverlay { proxy in
            GeometryReader { geometry in
                Rectangle()
                    .fill(Color.clear)
                    .contentShape(Rectangle())
                    .onContinuousHover { phase in
                        switch phase {
                        case .active(let location):
                            updateHoveredTimestamp(at: location,
                                                   in: geometry,
                                                   proxy: proxy)
                        case .ended:
                            hoveredTimestamp = nil
                        }
                    }
            }
        }
        // Floating annotation card rendered as an overlay so it can extend
        // outside the chart's plot area without clipping. Positioning is
        // approximate (top-trailing); a richer per-mark anchor is a Wave-5
        // polish item.
        .overlay(alignment: .topTrailing) {
            if let hoveredTimestamp,
               !samplesAtHoveredTimestamp(hoveredTimestamp).isEmpty {
                hoverCard(at: hoveredTimestamp)
                    .padding(.top, 8)
                    .padding(.trailing, 12)
                    .transition(.opacity)
            }
        }
        // AC-7: skip the implicit animation when Reduce Motion is on.
        .animation(reduceMotion ? nil : .easeInOut(duration: 0.2),
                   value: chartData)
    }

    // MARK: - Empty state (AC-4)

    @ViewBuilder
    private var emptyState: some View {
        if #available(macOS 14.0, *) {
            ContentUnavailableView(
                "No Data",
                systemImage: "chart.xyaxis.line",
                description: Text("No usage history in the selected range")
            )
        } else {
            // Hand-rolled fallback for macOS 13.0 (our deployment target).
            // Matches the visual weight of `ContentUnavailableView`: large
            // icon, bold headline, secondary subtitle, centered.
            VStack(spacing: 8) {
                Image(systemName: "chart.xyaxis.line")
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

    // MARK: - Hover helpers

    /// Translates a view-space hover location into the nearest sample's
    /// timestamp. We snap to the closest bucket boundary (rather than
    /// rendering at arbitrary x-positions) so the dashed RuleMark and the
    /// annotation always align with real data points.
    private func updateHoveredTimestamp(at location: CGPoint,
                                        in geometry: GeometryProxy,
                                        proxy: ChartProxy) {
        // `proxy.plotAreaFrame` only exists on macOS 14+; for macOS 13 we
        // fall back to using the geometry's own bounds, which is correct
        // when the chart fills the overlay (it does, via the Rectangle).
        let originX: CGFloat
        let plotWidth: CGFloat
        if #available(macOS 14.0, *) {
            let frame = geometry[proxy.plotAreaFrame]
            originX = frame.origin.x
            plotWidth = frame.size.width
        } else {
            originX = 0
            plotWidth = geometry.size.width
        }

        let xInPlot = location.x - originX
        guard xInPlot >= 0, xInPlot <= plotWidth else {
            hoveredTimestamp = nil
            return
        }

        guard let raw: Date = proxy.value(atX: xInPlot, as: Date.self) else {
            hoveredTimestamp = nil
            return
        }

        // Snap to closest sample timestamp so vertical line + annotation
        // line up with a real bucket. Uses `visibleSamples` (post-filter) so
        // when `selectedVendor` is non-nil the hover only locks onto buckets
        // that actually contain that vendor.
        let closest = visibleSamples
            .map(\.timestamp)
            .min(by: { abs($0.timeIntervalSince(raw)) < abs($1.timeIntervalSince(raw)) })
        hoveredTimestamp = closest
    }

    private func samplesAtHoveredTimestamp(_ timestamp: Date) -> [AggregatedSample] {
        visibleSamples.filter { $0.timestamp == timestamp }
    }

    // MARK: - Hover annotation card

    @ViewBuilder
    private func hoverCard(at timestamp: Date) -> some View {
        let samples = samplesAtHoveredTimestamp(timestamp)
        VStack(alignment: .leading, spacing: 4) {
            Text(timestamp.formatted(date: .abbreviated, time: .shortened))
                .font(.caption)
                .foregroundStyle(.secondary)
            ForEach(samples) { sample in
                HStack(spacing: 8) {
                    Circle()
                        .fill(chartVendorColor(for: sample.vendor))
                        .frame(width: 8, height: 8)
                    Text(sample.vendor)
                        .font(.caption)
                    Spacer()
                    Text(sample.costUsd.formatted(.currency(code: "USD")))
                        .font(.caption.monospacedDigit())
                }
            }
        }
        .padding(8)
        .background(
            RoundedRectangle(cornerRadius: 6)
                .fill(Color(nsColor: .windowBackgroundColor))
                .shadow(radius: 2)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 6)
                .stroke(Color.secondary.opacity(0.2), lineWidth: 0.5)
        )
        .frame(maxWidth: 220)
    }
}

// MARK: - Preview (AC-10 + T6 perf inspection)

#Preview("7 days × 5 vendors (mock)") {
    StackedAreaChart(chartData: .previewMock7Days)
        .padding()
        .frame(width: 900, height: 500)
}

#Preview("Empty state") {
    StackedAreaChart(chartData: .empty)
        .padding()
        .frame(width: 900, height: 500)
}

// MARK: - Preview mock data

private extension ChartData {
    /// 7 days × 5 vendors × 24 buckets/day = 840 samples — matches the
    /// worst-case 7d-hourly bucket count noted in story risks table. T6
    /// uses this preview to eyeball that 840 AreaMarks render without
    /// jank; if frame drops appear, we'd add `.chartPlotStyle { $0.compositingGroup() }`
    /// to the chart body and document it in the Change Log.
    static var previewMock7Days: ChartData {
        let vendors = ["openrouter", "anthropic", "openai", "zai", "gemini"]
        let calendar = Calendar.current
        let now = Date()
        let dayStart = calendar.startOfDay(for: now)

        var samples: [AggregatedSample] = []
        for dayOffset in 0..<7 {
            guard let day = calendar.date(byAdding: .day, value: -dayOffset, to: dayStart) else {
                continue
            }
            for hour in 0..<24 {
                guard let hourTs = calendar.date(byAdding: .hour, value: hour, to: day) else {
                    continue
                }
                for (idx, vendor) in vendors.enumerated() {
                    // Deterministic-but-varied mock cost: each vendor has a
                    // distinct base + a hour-of-day sinusoidal modulation.
                    let base = 0.05 * Double(idx + 1)
                    let hourMod = sin(Double(hour) / 24.0 * 2 * .pi) * 0.02 + 0.02
                    samples.append(AggregatedSample(
                        timestamp: hourTs,
                        vendor: vendor,
                        costUsd: base + hourMod,
                        requestCount: 10 + idx * 3
                    ))
                }
            }
        }
        return ChartData(samples: samples, vendors: vendors.sorted())
    }
}
