//
//  BudgetBurn.swift
//  Torven
//
//  Story 4.6 (Wave 4) — Monthly budget burn indicator. Surfaces month-to-date
//  spending against the user's configured `[budgets]` cap in
//  `~/.config/torven/config.toml`, calculated server-side by
//  `get_budget_status()` (see `crates/torven-core/src/uniffi_exports.rs`).
//
//  ## Layout decision (AC-6)
//
//  This view is mounted in the rodapé (footer) of `MainWindowView`'s
//  `ChartContent`, below the chart body and any inline error band. Two
//  alternatives were considered and rejected:
//
//    - Sidebar — would require restructuring `MainWindowView` into an
//      HSplitView, doubling the layout surface for a 60pt indicator.
//      Disproportionate scope for Wave 4's "stable shell + first charts"
//      goal (Decision WAVE4-D6 documented in the Story 4.1 plan).
//
//    - Top bar — already crowded with DateRangePicker + ViewMode +
//      ChartMetric in 4.5; another control there breaks the single-line
//      density budget at the 900pt default width.
//
//  Footer fits naturally between the chart body and the error banner,
//  occupies a fixed ~52pt strip, and degrades cleanly to `EmptyView()` when
//  the user has no `[budgets]` section configured (AC-5).
//
//  ## macOS 13 vs 14 Gauge (Risk #2)
//
//  `Gauge` was introduced for macOS 12, but the `.accessoryLinear` style
//  (the only horizontal gauge style with a colored fill we can tint
//  conditionally) requires macOS 13.0 — which IS our deployment target. Even
//  so, we wrap in `if #available(macOS 14.0, *)` and fall back to
//  `ProgressView` for two reasons:
//    1. Belt-and-suspenders: if a future Xcode SDK ever raises the Gauge
//       requirement (it has happened twice already across watchOS/iOS), the
//       fallback survives.
//    2. ProgressView has a more predictable tinting story on 13 (the
//       documented `.tint()` works), whereas `.gaugeStyle(.accessoryLinear)`
//       on 13.0 has reported edge cases with custom tints. Using ProgressView
//       on 13 sidesteps that.
//
//  ## Reduce Motion (AC-9)
//
//  The animation between burn-color thresholds (verde → amarelo → vermelho)
//  is suppressed when `accessibilityReduceMotion` is true. Without this, the
//  fill color crossfades on every refresh — visually noisy for users who
//  opted into Reduce Motion.
//

import SwiftUI

// MARK: - Math helper

/// Clamp a `Double` into a closed range. SwiftUI's `Gauge` API silently
/// pegs values to the range bounds, but `ProgressView(value:total:)` divides
/// the two without clamping — explicit clamping keeps the fallback path
/// well-defined for over-budget (>100%) and impossible-negative values.
///
/// File-private to avoid polluting the global namespace with a generic-named
/// extension; another file that wants the same primitive can re-declare or
/// promote to shared scope when it materialises.
extension Double {
    fileprivate func clampedToUnitPercentage() -> Double {
        if isNaN { return 0 }
        return Swift.min(Swift.max(self, 0), 100)
    }
}

// MARK: - BudgetBurn view

struct BudgetBurn: View {
    let status: BudgetStatus

    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        // AC-5: no-op when the user has not configured `[budgets]`. We return
        // EmptyView() so this site occupies zero layout (the parent's
        // `Divider()` is also conditional below).
        if status.hasBudget {
            content
        } else {
            EmptyView()
        }
    }

    private var content: some View {
        VStack(spacing: 4) {
            header
            burnBar
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        // AC-9: skip the implicit color crossfade when Reduce Motion is on.
        // Animating on `totalPercentUsed` ensures we only fire on actual
        // value drift, not on every body reevaluation.
        .animation(reduceMotion ? nil : .easeInOut(duration: 0.25),
                   value: status.totalPercentUsed)
    }

    // MARK: - Subviews

    private var header: some View {
        HStack {
            Text("Budget")
                .font(.caption.bold())
                .foregroundStyle(.secondary)
            Spacer()
            Text(spendLabel)
                .font(.caption.monospacedDigit())
                .foregroundStyle(.primary)
        }
    }

    /// Horizontal progress bar with a burn-color fill. Gauge on macOS 14+,
    /// ProgressView fallback on 13.0 (see header doc Risk #2 discussion).
    @ViewBuilder
    private var burnBar: some View {
        let value = status.totalPercentUsed.clampedToUnitPercentage()
        if #available(macOS 14.0, *) {
            Gauge(value: value, in: 0 ... 100) {
                EmptyView()
            }
            .gaugeStyle(.accessoryLinear)
            .tint(burnColor)
        } else {
            ProgressView(value: value, total: 100)
                .progressViewStyle(.linear)
                .tint(burnColor)
        }
    }

    // MARK: - Derived values

    /// Format: `$30.50 / $100.00` (or `$30.50 / —` when global budget is
    /// nil, which happens when the user only configured per-vendor caps).
    private var spendLabel: String {
        let spent = formatUsd(status.totalSpentUsd)
        guard let budget = status.totalBudgetUsd else {
            return "\(spent) / —"
        }
        return "\(spent) / \(formatUsd(budget))"
    }

    /// Burn-color thresholds per AC-5:
    /// - < 80%   → green (under budget, healthy)
    /// - 80-99%  → orange (approaching cap, warning)
    /// - >= 100% → red (over budget)
    ///
    /// `f.isNaN` is treated as green (no budget set yet => default neutral),
    /// `f.isInfinite` (zero-budget overspend) is treated as red.
    private var burnColor: Color {
        let p = status.totalPercentUsed
        if p.isNaN { return .green }
        if p.isInfinite { return .red }
        if p < 80 { return .green }
        if p < 100 { return .orange }
        return .red
    }

    /// Locale-stable USD formatter. We use the modern `Double.formatted(_:)`
    /// API (iOS 15 / macOS 12+) which lazy-creates a cached NumberFormatter
    /// under the hood. `currency(code: "USD")` keeps the symbol locale-aware
    /// while preserving USD semantics (the AC-1 contract is "USD only").
    private func formatUsd(_ amount: Double) -> String {
        amount.formatted(.currency(code: "USD"))
    }
}

// MARK: - Previews (AC-10 optional)

#Preview("Green — 50% used") {
    BudgetBurn(status: BudgetStatus(
        totalSpentUsd: 50.0,
        totalBudgetUsd: 100.0,
        totalPercentUsed: 50.0,
        perVendor: [],
        hasBudget: true
    ))
    .frame(width: 600)
    .padding()
}

#Preview("Amber — 85% used") {
    BudgetBurn(status: BudgetStatus(
        totalSpentUsd: 85.0,
        totalBudgetUsd: 100.0,
        totalPercentUsed: 85.0,
        perVendor: [],
        hasBudget: true
    ))
    .frame(width: 600)
    .padding()
}

#Preview("Red — 115% over budget") {
    BudgetBurn(status: BudgetStatus(
        totalSpentUsd: 115.0,
        totalBudgetUsd: 100.0,
        totalPercentUsed: 115.0,
        perVendor: [
            VendorBudgetStatus(
                vendorId: "openrouter",
                spentUsd: 60.0,
                budgetUsd: 50.0,
                percentUsed: 120.0
            ),
        ],
        hasBudget: true
    ))
    .frame(width: 600)
    .padding()
}

#Preview("Hidden — no budget configured") {
    BudgetBurn(status: BudgetStatus(
        totalSpentUsd: 0.0,
        totalBudgetUsd: nil,
        totalPercentUsed: 0.0,
        perVendor: [],
        hasBudget: false
    ))
    .frame(width: 600)
    .padding()
}
