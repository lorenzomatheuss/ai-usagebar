//
//  MenuBarContent.swift
//  Torven
//
//  ADR §4 — MenuBarExtra content wrapper. Story 2.1 introduces this layer
//  of indirection so Story 2.3 can swap in PopoverView without touching
//  TorvenApp.swift. For Wave 2, this delegates to the Story 1.5 stub
//  preserved under Views/Preservation/.
//

import SwiftUI

struct MenuBarContent: View {
    var body: some View {
        // Story 2.1: delegate to the Story 1.5 stub until Story 2.3 replaces it with PopoverView.
        VendorListView()
    }
}
