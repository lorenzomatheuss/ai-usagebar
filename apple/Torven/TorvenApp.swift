import SwiftUI

@main
struct TorvenApp: App {
    var body: some Scene {
        MenuBarExtra("Torven", systemImage: "chart.bar.fill") {
            Text("Torven — hello world")
        }
        .menuBarExtraStyle(.window)
    }
}
