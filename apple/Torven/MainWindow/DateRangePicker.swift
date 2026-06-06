//
//  DateRangePicker.swift
//  Torven
//
//  Story 4.2 (Wave 4): segmented period picker (7d / 30d / Custom) bound to
//  the `DateRange` value the rest of the Main Window observes. Decision
//  WAVE4-D3 (cravada): native `Picker`+`DatePicker.compact` — no SPM dep.
//
//  This view is intentionally state-holder agnostic — it takes a
//  `@Binding<DateRange>` and updates it on segment change (7d/30d) or on
//  the explicit "Apply" button (custom). The owning `MainWindowViewModel`
//  republishes the change so future ChartViewModels (4.3-4.5) react via
//  Combine.
//
//  AC-6 (ADR-4 FFI surface): `DateRange.sinceTs` and `.untilTs` are Unix
//  timestamps in milliseconds (i64) ready to be passed to
//  `ffi_query_aggregated`. Conversion lives here in Swift; Rust side stays
//  in `Int64` ms per ADR-4.
//

import SwiftUI

// MARK: - Mode

enum DateRangeMode: String, CaseIterable, Identifiable {
    case days7
    case days30
    case custom

    var id: String { rawValue }

    var label: String {
        switch self {
        case .days7: return "7d"
        case .days30: return "30d"
        case .custom: return "Custom"
        }
    }
}

// MARK: - Value type

struct DateRange: Equatable {
    let startDate: Date
    let endDate: Date

    // ADR-4 FFI surface: Unix epoch in *milliseconds* as i64 — matches the
    // `since_ts: i64 (ms)` parameter on `ffi_query_aggregated` in Story 4.0.5.
    var sinceTs: Int64 { Int64(startDate.timeIntervalSince1970 * 1000) }
    var untilTs: Int64 { Int64(endDate.timeIntervalSince1970 * 1000) }

    /// AC-5: covers `00:00 D-7` → `23:59:59 D` in the local calendar.
    static func last7Days(from reference: Date = Date(),
                          calendar: Calendar = .current) -> DateRange {
        Self.lastNDays(7, from: reference, calendar: calendar)
    }

    static func last30Days(from reference: Date = Date(),
                           calendar: Calendar = .current) -> DateRange {
        Self.lastNDays(30, from: reference, calendar: calendar)
    }

    private static func lastNDays(_ n: Int,
                                  from reference: Date,
                                  calendar: Calendar) -> DateRange {
        let startOfToday = calendar.startOfDay(for: reference)
        // Force-unwraps below are safe: `byAdding:` only fails on calendar
        // overflow (year ±10000+) which doesn't happen with single-digit
        // day offsets from `now`.
        let start = calendar.date(byAdding: .day, value: -n, to: startOfToday)!
        let endOfToday = calendar.endOfDay(for: reference)
        return DateRange(startDate: start, endDate: endOfToday)
    }
}

// Thin shim over `onChange` that picks the macOS 14+ two-arg form when
// available and falls back to the macOS 13 single-arg one otherwise. Avoids
// the `deprecated-declaration` warning at our deployment target (13.0).
private extension View {
    @ViewBuilder
    func onValueChange<T: Equatable>(of value: T,
                                     perform action: @escaping (T) -> Void) -> some View {
        if #available(macOS 14.0, *) {
            self.onChange(of: value) { _, newValue in action(newValue) }
        } else {
            self.onChange(of: value, perform: action)
        }
    }
}

private extension Calendar {
    /// `23:59:59.999` of the given day in this calendar's timezone.
    func endOfDay(for date: Date) -> Date {
        var comps = DateComponents()
        comps.day = 1
        comps.second = -1
        return self.date(byAdding: comps, to: startOfDay(for: date))!
    }
}

// MARK: - View

struct DateRangePicker: View {
    @Binding var dateRange: DateRange

    @State private var mode: DateRangeMode = .days7
    @State private var pendingStart: Date = Date()
    @State private var pendingEnd: Date = Date()

    private var isCustomInvalid: Bool { pendingEnd < pendingStart }

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 12) {
            Picker("Period", selection: $mode) {
                ForEach(DateRangeMode.allCases) { m in
                    Text(m.label).tag(m)
                }
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .frame(width: 220)
            .onValueChange(of: mode) { newMode in
                handleModeChange(newMode)
            }

            if mode == .custom {
                customRangeControls
            }

            Spacer(minLength: 0)
        }
    }

    @ViewBuilder
    private var customRangeControls: some View {
        DatePicker("start:", selection: $pendingStart, displayedComponents: .date)
            .datePickerStyle(.compact)
            .labelsHidden()
        Text("→").foregroundStyle(.secondary)
        DatePicker("end:", selection: $pendingEnd, displayedComponents: .date)
            .datePickerStyle(.compact)
            .labelsHidden()

        Button("Apply") {
            applyCustomRange()
        }
        .disabled(isCustomInvalid)

        if isCustomInvalid {
            Text("End date must be after start date")
                .font(.caption)
                .foregroundColor(.red)
        }
    }

    private func handleModeChange(_ newMode: DateRangeMode) {
        switch newMode {
        case .days7:
            dateRange = .last7Days()
        case .days30:
            dateRange = .last30Days()
        case .custom:
            // Seed the local edit fields from the current range so the user
            // starts from whatever was previously applied.
            pendingStart = dateRange.startDate
            pendingEnd = dateRange.endDate
        }
    }

    private func applyCustomRange() {
        let cal = Calendar.current
        let start = cal.startOfDay(for: pendingStart)
        let end = cal.endOfDay(for: pendingEnd)
        dateRange = DateRange(startDate: start, endDate: end)
    }
}

#Preview("Default 7d") {
    StatefulPreview(initial: DateRange.last7Days())
        .frame(width: 700, height: 60)
        .padding()
}

private struct StatefulPreview: View {
    @State var range: DateRange
    init(initial: DateRange) { _range = State(initialValue: initial) }
    var body: some View { DateRangePicker(dateRange: $range) }
}
