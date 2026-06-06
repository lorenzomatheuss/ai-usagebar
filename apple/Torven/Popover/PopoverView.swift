//
//  PopoverView.swift
//  Torven
//
//  Story 2.3 (Wave 2): real popover view — header + empty container.
//  Story 3.1 (Wave 3): injects @EnvironmentObject coreBridge + single
//  OpenRouter VendorCard (proof of integration).
//  Story 3.2 (Wave 3): scales to ForEach(coreBridge.vendors) rendering
//  all 5 cards in canonical get_vendor_list() order (Anthropic, OpenAI,
//  OpenRouter, Z.AI, Gemini) and closes UX-Q1 empirically (see ADR §8).
//  Story 4.1 (Wave 4): adds the "Show History…" footer button — first
//  vector of the dual invocation pattern (WAVE4-D1, cravada). Routes
//  to MainWindowController.shared.show(); the ⌘1 global hotkey is the
//  second vector (registered in TorvenApp.swift).
//
//  UX-Q1 CLOSED 2026-06-04: 380×540 adopted — 360×420 forced the last
//  card (Gemini) to be clipped/scrolled. See ADR §8 for the closure note.
//

import SwiftUI

struct PopoverView: View {
    @EnvironmentObject var coreBridge: TorvenCoreBridge

    var body: some View {
        VStack(spacing: 0) {
            Text("Torven")
                .font(.title3)
                .fontWeight(.semibold)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 16)
                .padding(.vertical, 12)

            VStack(spacing: 8) {
                ForEach(coreBridge.vendors, id: \.id) { vendor in
                    VendorCard(vendor: vendor)
                }
            }
            .padding(.horizontal, 16)

            Spacer(minLength: 0)

            HStack {
                Spacer()
                Button("Show History…") {
                    MainWindowController.shared.show()
                }
                .buttonStyle(.plain)
                .font(.footnote)
                .foregroundColor(.secondary)
                Spacer()
            }
            .padding(.vertical, 8)
        }
        .frame(width: 380, height: 540)
    }
}
