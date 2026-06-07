//
//  AggregatedSample.swift
//  Torven
//
//  Story 4.3 (Wave 4): chart-domain types that translate the `TimeBucket[]`
//  output of `ffi_query_aggregated` (Story 4.0.5) into the shape Swift Charts
//  consumes in `StackedAreaChart` (and, later, Stories 4.4/4.5/4.6).
//
//  Why a Swift-side wrapper at all (instead of binding the chart directly to
//  `[TimeBucket]`)?
//  - `TimeBucket` carries fields Swift Charts doesn't use (tokens_sum,
//    metric_kind, account_id, end_ts) — wrapping keeps the chart's data
//    surface narrow and intention-revealing.
//  - `Identifiable` is a Chart-API expectation that `TimeBucket` (a UDL-
//    generated struct) doesn't provide; rolling our own `id: UUID` here is
//    cheaper than extending an autogen type.
//  - Future stories (4.5 tokens chart, 4.4 mini-charts) can re-derive
//    different `AggregatedSample` projections from the same `TimeBucket[]`
//    without coupling each chart to the FFI surface.
//
//  Why `BucketStrategy` is *not* redeclared here: the UDL-generated enum in
//  `apple/Torven/Bridge/Generated/torven_core.swift` (Story 4.0.5) is the
//  single source of truth. Redeclaring would force a manual mapping at the
//  FFI boundary and silently drift if the Rust enum ever grows a variant.
//

import Charts
import Foundation
import SwiftUI

// MARK: - Vendor color mapping (shared chart infra)

/// Brand-aligned colors for the 4 supported vendors. Originally defined in
/// `StackedAreaChart.swift` (Story 4.3); promoted to shared scope in Story 4.4
/// so `MiniVendorChart` (per-vendor grid) and `PerVendorGrid` (selection
/// highlight border) can reuse the same palette without duplicating the
/// declaration. Story 5.5.1 (WAVE5.5-D1) dropped the reserved Gemini slot —
/// the entry was confusing the chart legend during live smoke and Gemini
/// is no longer in the v1.0 vendor list.
///
/// `KeyValuePairs` (not `[String: Color]`) because Swift Charts consumes
/// `chartForegroundStyleScale` order-sensitively — the legend renders in
/// declaration order. A dictionary's iteration order isn't guaranteed.
///
/// `internal` (default) visibility — accessible across all chart files in the
/// `Torven` target. `lowercase` keys match the vendor strings returned by
/// `ffi_query_aggregated` (Story 4.0.5).
let chartVendorColorMapping: KeyValuePairs<String, Color> = [
    "openrouter": Color(red: 0.55, green: 0.36, blue: 0.96),  // purple
    "anthropic":  Color(red: 0.94, green: 0.49, blue: 0.22),  // orange
    "openai":     Color(red: 0.10, green: 0.52, blue: 0.36),  // dark green
    "zai":        Color(red: 0.20, green: 0.51, blue: 0.93),  // blue
]

/// Looks up a vendor's chart color from `chartVendorColorMapping`. Returns
/// gray if the vendor wasn't pre-registered (defensive — should never trip
/// since `ChartData.vendors` is derived from the same FFI vendor strings
/// that the palette enumerates).
func chartVendorColor(for vendor: String) -> Color {
    for (key, value) in chartVendorColorMapping where key == vendor {
        return value
    }
    return .gray
}

/// Human-readable label for a vendor id. Used by `MiniVendorChart` header
/// (Story 4.4) and any other chart that needs a display name without paying
/// the cost of an FFI lookup into `VendorInfo`. Kept as a small switch
/// (not a dictionary) so the compiler can warn if a new vendor id is added
/// to the palette without a matching display name.
func chartVendorDisplayName(_ vendorId: String) -> String {
    // Story 5.5.1 (WAVE5.5-D1) removed the explicit Gemini case. The
    // `default` fallback handles any future/unknown vendor slug — including
    // the unlikely case that legacy persisted data still references the old
    // Gemini slug — by Title-casing the slug rather than rendering an empty
    // label.
    switch vendorId {
    case "openrouter": return "OpenRouter"
    case "anthropic":  return "Anthropic"
    case "openai":     return "OpenAI"
    case "zai":        return "Z.AI"
    default:           return vendorId.capitalized
    }
}

// MARK: - AggregatedSample

/// One stacked-area data point: a single (bucket, vendor) pair carrying the
/// cost that period contributed for that vendor.
///
/// AC-1 surface. Note: `id` is intentionally a fresh `UUID` per instance —
/// `(bucketStartTs, vendor)` is the natural composite key but Swift Charts
/// doesn't need stable identity across reloads (the whole `samples` array is
/// replaced wholesale), so a UUID avoids the boilerplate of a custom
/// `Hashable` composite key.
struct AggregatedSample: Identifiable, Equatable {
    let id: UUID
    let timestamp: Date
    let vendor: String
    let costUsd: Double
    let requestCount: Int

    init(id: UUID = UUID(),
         timestamp: Date,
         vendor: String,
         costUsd: Double,
         requestCount: Int) {
        self.id = id
        self.timestamp = timestamp
        self.vendor = vendor
        self.costUsd = costUsd
        self.requestCount = requestCount
    }
}

// MARK: - ChartData

/// Aggregate envelope for everything `StackedAreaChart` needs to render.
///
/// `vendors` is a derived, alphabetically-sorted, de-duplicated list of the
/// vendors present in `samples`. Charts that need a deterministic legend
/// order (and the `vendorColorMapping` lookup) read from here rather than
/// re-scanning `samples` on every diff.
struct ChartData: Equatable {
    let samples: [AggregatedSample]
    let vendors: [String]

    /// Returns `true` when the underlying query produced no buckets — the
    /// view layer routes this to `ContentUnavailableView` (or its macOS-13
    /// fallback) per AC-4.
    var isEmpty: Bool { samples.isEmpty }

    static let empty = ChartData(samples: [], vendors: [])
}

// MARK: - Chart interpolation helper (Story 5.5.1 / WAVE5.5-D3)

/// Picks the right Swift Charts `InterpolationMethod` for a given sample
/// count. `.catmullRom` requires **at least 2 data points** to produce a
/// spline — feeding it a single sample renders an empty chart, which live
/// smoke on 2026-06-07 surfaced as ISSUE-C (a "1 day of data" run looked
/// indistinguishable from "no data"). Falling back to `.linear` for the
/// degenerate case lets a single point render as a flat segment while
/// preserving the catmullRom curve everywhere it's well-defined.
///
/// `internal` (default) visibility — both `StackedAreaChart` (which holds
/// a `ChartData`) and `MiniVendorChart` (which holds a pre-filtered
/// `[AggregatedSample]`) need to consult this with their respective input
/// shapes, so we expose two convenience entry points.
extension ChartData {
    /// Interpolation method to use when rendering the aggregated samples of
    /// this `ChartData`. See the file-level docs above for the rationale.
    var interpolation: InterpolationMethod {
        samples.count >= 2 ? .catmullRom : .linear
    }
}

extension Array where Element == AggregatedSample {
    /// Same `samples.count >= 2 ? .catmullRom : .linear` rule applied to a
    /// pre-filtered per-vendor sample array. Used by `MiniVendorChart`
    /// (which never sees the outer `ChartData`) so a single-point vendor
    /// card doesn't render empty inside an otherwise multi-point grid.
    var chartInterpolation: InterpolationMethod {
        count >= 2 ? .catmullRom : .linear
    }
}

// MARK: - FFI translation

extension AggregatedSample {
    /// Translate the `[TimeBucket]` output of `ffi_query_aggregated` (Story
    /// 4.0.5) into `ChartData`. Story 4.0.5 AC-5 guarantees the Rust query
    /// returns buckets in ascending `bucket_start_ts` order, so we preserve
    /// that ordering here (Swift Charts respects the array's order when
    /// building the x-axis domain).
    ///
    /// Why divide by 1000.0 (not 1000)? `bucket_start_ts` is `Int64` ms; the
    /// `Date(timeIntervalSince1970:)` initializer takes `TimeInterval`
    /// (Double seconds). Integer division would drop the ms remainder and
    /// snap every bucket to the second boundary — visually identical for
    /// hourly/daily/weekly buckets but technically lossy and easy to forget.
    static func fromFFI(_ buckets: [TimeBucket]) -> ChartData {
        let samples = buckets.map { bucket -> AggregatedSample in
            AggregatedSample(
                timestamp: Date(timeIntervalSince1970: Double(bucket.bucketStartTs) / 1000.0),
                vendor: bucket.vendor,
                costUsd: bucket.costSumUsd,
                requestCount: Int(bucket.requestCount)
            )
        }

        // Deterministic vendor list for the legend + color scale. Sorted
        // alphabetically so the legend order is stable across reloads
        // regardless of which vendor happens to appear first in a given
        // range's buckets.
        var seen = Set<String>()
        var vendors: [String] = []
        for bucket in buckets where seen.insert(bucket.vendor).inserted {
            vendors.append(bucket.vendor)
        }
        vendors.sort()

        return ChartData(samples: samples, vendors: vendors)
    }
}

// MARK: - ChartMetric (Story 4.5)

/// Selects which numeric dimension of `AggregatedSample` the chart layer
/// renders on its Y-axis: cumulative USD cost or raw request count.
///
/// Co-located with the other chart-domain types (rather than promoted to its
/// own file) because:
///   1. `StackedAreaChart`, `MiniVendorChart`, and `PerVendorGrid` all reach
///      for both `AggregatedSample` and `ChartMetric` in the same scope —
///      keeping them in one file means a single `import` line for callers.
///   2. The enum is structurally trivial (two cases, two computed strings)
///      and unlikely to grow before Wave 5 introduces tokens/latency metrics.
///      When that happens, a dedicated `ChartMetric.swift` becomes warranted.
///
/// `String`-backed raw values double as the segmented Picker's user-facing
/// label (no localization yet — v1.0 ships English-only per ADR-2).
enum ChartMetric: String, CaseIterable, Identifiable {
    case cost = "Cost"
    case requests = "Requests"

    var id: String { rawValue }

    /// Y-axis title used by both `StackedAreaChart` and `MiniVendorChart`.
    /// Cost keeps the "(USD)" suffix to match the legacy 4.3 chart; requests
    /// is a unitless count so the bare metric name suffices.
    var yAxisLabel: String {
        switch self {
        case .cost: return "Cost (USD)"
        case .requests: return "Requests"
        }
    }
}
