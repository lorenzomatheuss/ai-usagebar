//
//  MainWindowView.swift
//  Torven
//
//  Story 4.1 (Wave 4): placeholder SwiftUI shell for the Main Window.
//  Hosted by MainWindowController via NSHostingView. The real content
//  arrives in:
//    - Story 4.2 — Date Range Picker (header row)
//    - Story 4.3 — Cost-by-vendor stacked bar chart
//    - Story 4.4 — Tokens / Requests line charts
//    - Story 4.5 — Budget burn-down meter
//    - Story 4.6 — Empty/loading/error states
//
//  This shell is intentionally minimal — only an empty `Text` so the
//  900×600 window has visible content and the build link is exercised.
//

import SwiftUI

struct MainWindowView: View {
    var body: some View {
        Text("History — Wave 4")
            .font(.title2)
            .foregroundStyle(.secondary)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

#Preview {
    MainWindowView()
        .frame(width: 900, height: 600)
}
