//
//  VendorCard.swift
//  Torven
//
//  Story 3.1 (Wave 3): single vendor card — parametrized by VendorInfo,
//  placeholders for cost/budget/sparkline (real metrics arrive Wave 4
//  via history snapshots + Swift Charts).
//
//  Story 3.2 wraps this view in a ForEach(coreBridge.vendors) so all 5
//  vendors render. Story 3.3 adds tap → AccountPicker inline sheet.
//
//  Story 3.3 (Wave 3): the card is now tappable. On tap an inline sheet
//  with the AccountPicker presents. Selecting an account drives a swap via
//  `TorvenCoreBridge.swapActiveAccount(...)`. Vendors without configured
//  accounts present the picker showing an empty-state message (per AC-5).
//
//  Constructor injection (`let vendor: VendorInfo`) is preserved — the
//  picker accesses TorvenCoreBridge via @EnvironmentObject only, not via
//  the init, so previews can keep mocking VendorInfo without needing the
//  bridge.
//

import SwiftUI

struct VendorCard: View {
    let vendor: VendorInfo

    @EnvironmentObject private var coreBridge: TorvenCoreBridge
    @State private var showPicker: Bool = false

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text(vendor.displayName)
                    .font(.headline)
                Spacer()
                Image(systemName: vendor.isConfigured ? "checkmark.circle.fill" : "circle")
                    .foregroundColor(vendor.isConfigured ? .green : .secondary)
            }
            HStack {
                // Travessão tipográfico (U+2014) sinaliza "dado pendente"
                // sem parecer bug. Wave 4 troca por valor real do
                // HistorySnapshot agregado por janela temporal.
                Text("$— this period")
                    .font(.subheadline)
                    .foregroundColor(.secondary)
                Spacer()
                Text("—% used")
                    .font(.subheadline)
                    .foregroundColor(.secondary)
            }
            // Sparkline placeholder — Wave 4 substitui por Swift Chart real.
            Rectangle()
                .fill(Color.secondary.opacity(0.15))
                .frame(height: 24)
        }
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .fill(Color.gray.opacity(0.08))
        )
        .contentShape(Rectangle())
        .onTapGesture { showPicker = true }
        .sheet(isPresented: $showPicker) {
            AccountPicker(
                vendor: vendor,
                accounts: coreBridge.accounts(for: vendor.id),
                onSelect: { accountId in
                    coreBridge.swapActiveAccount(vendorId: vendor.id, accountId: accountId)
                    showPicker = false
                },
                onDismiss: { showPicker = false }
            )
        }
    }
}

#Preview {
    VStack {
        VendorCard(vendor: VendorInfo(id: "openrouter", displayName: "OpenRouter", isConfigured: true))
        VendorCard(vendor: VendorInfo(id: "openai", displayName: "OpenAI", isConfigured: false))
    }
    .padding()
    .frame(width: 328)
    .environmentObject(TorvenCoreBridge())
}
