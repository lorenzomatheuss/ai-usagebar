//
//  MainWindowViewModel.swift
//  Torven
//
//  Story 4.2 (Wave 4): state holder for the Main Window. Owns the active
//  `DateRange` that downstream chart view models will observe via Combine
//  (`mainViewModel.$dateRange.sink { … }`).
//
//  Deliberately FFI-free in this story. The aggregated query
//  (`ffi_query_aggregated`, Story 4.0.5) is invoked from `ChartViewModel`s
//  introduced in Story 4.3+. Keeping this view model as a pure state holder
//  lets Story 4.2 ship without a dependency on 4.0.5 and keeps the range
//  selector trivially unit-testable.
//
//  Why a class (not a value-type Observable struct): we need reference
//  identity to share one instance among multiple chart sub-views, and the
//  Main Window currently deploys to macOS 13.0 where `@Observable` (macOS
//  14+) isn't an option.
//

import Combine
import Foundation

@MainActor
final class MainWindowViewModel: ObservableObject {
    @Published var dateRange: DateRange

    init(initialRange: DateRange = .last7Days()) {
        self.dateRange = initialRange
    }

    /// Replace the active range. Observers (`$dateRange` subscribers) react
    /// automatically — no explicit notification needed.
    func applyRange(_ range: DateRange) {
        dateRange = range
    }
}
