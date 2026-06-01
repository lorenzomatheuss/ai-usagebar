//
//  VendorListView.swift
//  Torven
//
//  Temporary popover content for Story 1.5 — proves the Rust → FFI → Swift
//  → SwiftUI loop works end-to-end by rendering the 5 vendors returned by
//  `getVendorList()`. Will be replaced by the real PopoverView in Story 10
//  (current-window snapshot + per-vendor usage cards).
//

import SwiftUI

struct VendorListView: View {
    @EnvironmentObject var coreBridge: TorvenCoreBridge

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Torven — Vendors")
                .font(.headline)

            // Stable id from the Rust side; the slug is guaranteed unique by
            // `get_vendor_list()` in torven-core (see uniffi_exports.rs).
            ForEach(coreBridge.vendors, id: \.id) { vendor in
                HStack {
                    Text(vendor.displayName)
                    Spacer()
                    if vendor.isConfigured {
                        Image(systemName: "checkmark.circle.fill")
                            .foregroundColor(.green)
                    } else {
                        Image(systemName: "circle")
                            .foregroundColor(.secondary)
                    }
                }
            }
        }
        .padding()
        .frame(width: 280)
    }
}
