//
//  VendorListView_Story15.swift  (originally VendorListView.swift)
//  Torven
//
//  Story 2.1 (Wave 2): preserved here as a regression artifact of the
//  Story 1.5 end-to-end FFI proof. Not used in the happy path from
//  Story 2.3 onward (PopoverView takes over). Kept under the original
//  struct name `VendorListView` so existing nm symbol audits and any
//  forthcoming snapshot tests continue to resolve it.
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
