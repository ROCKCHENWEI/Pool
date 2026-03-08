import SwiftUI
import PoolCore

@main
struct PoolApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 1400, height: 900)
    }
}
