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

import Foundation

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
