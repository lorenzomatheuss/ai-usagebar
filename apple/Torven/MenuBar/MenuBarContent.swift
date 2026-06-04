//
//  MenuBarContent.swift
//  Torven
//
//  ADR §4 — MenuBarExtra content wrapper. Story 2.1 introduced this layer
//  of indirection so the entry point could be decoupled from the content
//  view. Story 2.3 (Wave 2) replaces the Story 1.5 stub (`VendorListView`)
//  with the real `PopoverView`. Wave 3 will enrich `PopoverView` with
//  vendor cards; this wrapper does not need to change again until a future
//  story introduces conditional content (e.g. onboarding vs. main view).
//

import SwiftUI

struct MenuBarContent: View {
    var body: some View {
        // Story 2.3: replaced Story 1.5 stub (VendorListView, now in Views/Preservation/)
        // with PopoverView. Vendor cards enter in Wave 3 (Story 3.x).
        PopoverView()
    }
}
