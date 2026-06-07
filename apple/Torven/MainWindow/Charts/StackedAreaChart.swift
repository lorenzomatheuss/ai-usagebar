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
//  ## Vendor colors (4-vendor palette)
//
//  Brand-aligned where each vendor has a distinctive marketing color. Story
//  5.5.1 (WAVE5.5-D1) dropped the reserved Gemini slot from this palette —
//  the empty legend entry was confusing during live smoke. Picked from
//  `KeyValuePairs<String, Color>` rather than a dictionary so the iteration
//  order is deterministic (Swift Charts uses this order for both the
//  foreground style scale and the legend).
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

    /// Story 4.5 AC-2: chosen Y-axis dimension. Default `.cost` preserves the
    /// Story 4.3 callers' behaviour and avoids a breaking change when this
    /// view is summoned from preview blocks or older test sites.
    var metric: ChartMetric = .cost

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

    /// Story 4.5 AC-2: projects an `AggregatedSample` onto the current
    /// `metric`'s Y-axis value. Returning `Double` (even for the integer
    /// `requestCount`) keeps the `AreaMark` y-`.value(...)` builder happy
    /// without a separate generic chart path.
    private func metricValue(_ sample: AggregatedSample) -> Double {
        switch metric {
        case .cost: return sample.costUsd
        case .requests: return Double(sample.requestCount)
        }
    }

    /// Story 4.5: hover-card formatter mirroring `metricValue`. Returns the
    /// user-facing string for the current metric. Kept beside `metricValue`
    /// so future metrics (tokens, latency) only need to grow one switch.
    private func formattedMetricValue(for sample: AggregatedSample) -> String {
        switch metric {
        case .cost:
            return sample.costUsd.formatted(.currency(code: "USD"))
        case .requests:
            // Integer formatting — `requestCount` is already `Int`; using it
            // directly avoids a stringly-typed "%d" path. Trailing "requests"
            // word disambiguates the count from a currency value in the
            // hover popover.
            // FMT-001 (Wave 4 polish): `.grouping(.automatic)` matches the
            // Y-axis tick formatting so the hover card and the axis labels
            // both render large counts with locale-aware thousand
            // separators ("1.234" pt-BR / "1,234" en-US).
            let formatted = sample.requestCount
                .formatted(IntegerFormatStyle<Int>().grouping(.automatic))
            return "\(formatted) requests"
        }
    }

    // MARK: - Chart

    private var chartView: some View {
        Chart {
            ForEach(visibleSamples) { sample in
                AreaMark(
                    x: .value("Time", sample.timestamp),
                    y: .value(metric.yAxisLabel, metricValue(sample))
                )
                .foregroundStyle(by: .value("Vendor", sample.vendor))
                // Story 5.5.1 (WAVE5.5-D3 / ISSUE-C): `.catmullRom` needs
                // ≥2 points to compute a spline; the helper falls back to
                // `.linear` for the degenerate single-sample case so the
                // chart never renders empty when the user has only one
                // bucket of data (1-day range, brand-new install, etc.).
                .interpolationMethod(visibleSamples.chartInterpolation)
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
            // Story 4.5 AC-2: format flips with the active metric. Currency
            // for `.cost` keeps the legacy 4.3 axis exactly; integer for
            // `.requests` avoids "$" prefixes on a count value, which would
            // mislead the eye. `IntegerFormatStyle<Int>()` is the macOS-13-
            // safe way to spell "no decimals, no currency" — `Decimal.FormatStyle`
            // also works but pulls in a Foundation type the chart doesn't
            // otherwise need.
            //
            // FMT-001 (Wave 4 polish): `.grouping(.automatic)` adds locale-
            // aware thousand separators so request counts ≥1000 read cleanly
            // ("1.234" pt-BR / "1,234" en-US) instead of cramped digits.
            if metric == .cost {
                AxisMarks(format: .currency(code: "USD"))
            } else {
                AxisMarks(format: IntegerFormatStyle<Int>().grouping(.automatic))
            }
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
                    // Story 4.5: annotation reads the same value the chart is
                    // drawing — currency string for `.cost`, "N requests" for
                    // `.requests`. Without this branch the card would still
                    // show USD even while the chart's bars/areas were sized
                    // by request count, which is the kind of subtle mismatch
                    // that erodes user trust in the visualization.
                    Text(formattedMetricValue(for: sample))
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

#Preview("7 days × 5 vendors · Cost") {
    StackedAreaChart(chartData: .previewMock7Days, metric: .cost)
        .padding()
        .frame(width: 900, height: 500)
}

#Preview("7 days × 5 vendors · Requests") {
    StackedAreaChart(chartData: .previewMock7Days, metric: .requests)
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
    /// 7 days × 4 vendors × 24 buckets/day = 672 samples — matches the
    /// worst-case 7d-hourly bucket count noted in story risks table. T6
    /// uses this preview to eyeball that the AreaMarks render without
    /// jank; if frame drops appear, we'd add `.chartPlotStyle { $0.compositingGroup() }`
    /// to the chart body and document it in the Change Log. Story 5.5.1
    /// (WAVE5.5-D1) trimmed the vendor list from 5 → 4 (Gemini removed).
    static var previewMock7Days: ChartData {
        let vendors = ["openrouter", "anthropic", "openai", "zai"]
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
