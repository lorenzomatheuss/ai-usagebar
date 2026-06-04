//
//  PopoverView.swift
//  Torven
//
//  Story 2.3 (Wave 2): real popover view — header + empty container.
//  Vendor cards enter in Wave 3 (Stories 3.x). Frame 360×420 is the
//  starting point; UX-Q1 (Wave 3) may revise to 380×540 if cards spill.
//  Next story to modify this file: first Story 3.x that adds VendorCard.
//

import SwiftUI

struct PopoverView: View {
    var body: some View {
        VStack(spacing: 0) {
            Text("Torven")
                .font(.title3)
                .fontWeight(.semibold)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 16)
                .padding(.vertical, 12)
            Spacer()
        }
        .frame(width: 360, height: 420)
    }
}
