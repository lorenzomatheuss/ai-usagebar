//
//  MainWindowView.swift
//  Torven
//
//  Wave 4 Main Window root. Owns the `MainWindowViewModel` and lays out:
//    - top bar : DateRangePicker (Story 4.2)
//                 + future Cost/Requests toggle (Story 4.5)
//    - content : empty Spacer for now — Stories 4.3-4.6 add the charts
//                that observe `viewModel.$dateRange`.
//

import SwiftUI

struct MainWindowView: View {
    @StateObject private var viewModel = MainWindowViewModel()

    var body: some View {
        VStack(spacing: 0) {
            topBar
                .padding(.horizontal, 16)
                .padding(.vertical, 10)

            Divider()

            contentArea
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var topBar: some View {
        HStack(spacing: 12) {
            DateRangePicker(dateRange: $viewModel.dateRange)
            Spacer()
        }
    }

    private var contentArea: some View {
        // Placeholder until Story 4.3 wires the cost-by-vendor chart.
        VStack {
            Text("History — Wave 4")
                .font(.title2)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

#Preview {
    MainWindowView()
        .frame(width: 900, height: 600)
}
