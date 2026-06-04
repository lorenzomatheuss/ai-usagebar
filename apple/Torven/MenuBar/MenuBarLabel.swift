//
//  MenuBarLabel.swift
//  Torven
//
//  ADR §4 — NSStatusItem custom view content. Story 2.2 (Wave 2) introduces
//  this struct as a configurable skeleton: the SF Symbol is a parameter so
//  Wave 7 (AR-8 spike) can swap it for a dynamic label driven by vendor
//  pressure without touching TorvenApp.swift.
//
//  NOT a dynamic label yet — no Rust core dependency. Static symbol only.
//  See AR-8 in docs/architecture/torven-v1-adr.md §7 for the Wave 7 plan.
//

import SwiftUI

struct MenuBarLabel: View {
    static let defaultSymbol = "chart.bar.fill"

    let symbol: String

    var body: some View {
        Image(systemName: symbol)
    }
}
