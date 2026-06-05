//
//  AccountPicker.swift
//  Torven
//
//  Story 3.3 (Wave 3): inline sheet picker that lists the accounts configured
//  for a vendor with the active marker. Tap a row → onSelect(account.id).
//  Cancel → onDismiss(). The view is parameterized so VendorCard can drive
//  presentation via .sheet(isPresented:) without coupling to TorvenCoreBridge.
//
//  Wave 4 may introduce keychain credential validation feedback inline (e.g.
//  "Key expired") — that addition lives in this file, not the bridge.
//
//  Accessibility: respects Reduce Motion via @Environment(\.accessibilityReduceMotion).
//  The native macOS .sheet() container already handles slide presentation
//  through system controls; the explicit .transition modifier here ensures
//  the content opacity-fade path is taken when Reduce Motion is enabled.
//

import SwiftUI

struct AccountPicker: View {
    let vendor: VendorInfo
    let accounts: [AccountInfo]
    let onSelect: (String) -> Void
    let onDismiss: () -> Void

    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text("Switch account for \(vendor.displayName)")
                    .font(.headline)
                Spacer()
                Button("Cancel", action: onDismiss)
                    .buttonStyle(.borderless)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)

            Divider()

            if accounts.isEmpty {
                Text("No accounts configured for this vendor.")
                    .font(.subheadline)
                    .foregroundColor(.secondary)
                    .padding(16)
                    .frame(maxWidth: .infinity, alignment: .leading)
            } else {
                ScrollView {
                    VStack(spacing: 0) {
                        ForEach(accounts, id: \.id) { account in
                            Button(action: { onSelect(account.id) }) {
                                HStack {
                                    Text(account.label)
                                        .foregroundColor(.primary)
                                    Spacer()
                                    if account.isActive {
                                        Image(systemName: "checkmark")
                                            .foregroundColor(.accentColor)
                                    }
                                }
                                .contentShape(Rectangle())
                                .padding(.horizontal, 16)
                                .padding(.vertical, 10)
                            }
                            .buttonStyle(.plain)
                            Divider()
                        }
                    }
                }
            }
        }
        .frame(minWidth: 320, idealWidth: 360, minHeight: 180)
        .transition(reduceMotion ? .opacity : .move(edge: .bottom))
        .animation(reduceMotion ? .linear(duration: 0.1) : .default, value: accounts)
    }
}

#Preview {
    AccountPicker(
        vendor: VendorInfo(id: "openrouter", displayName: "OpenRouter", isConfigured: true),
        accounts: [
            AccountInfo(id: "openrouter-personal", label: "Personal", isActive: true),
            AccountInfo(id: "openrouter-clienteacme", label: "ClienteAcme", isActive: false),
        ],
        onSelect: { _ in },
        onDismiss: {}
    )
}
