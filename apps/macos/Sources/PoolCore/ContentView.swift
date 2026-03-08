import SwiftUI

public struct ContentView: View {
    @State private var selectedTab = "timeline"

    public init() {}

    public var body: some View {
        NavigationSplitView {
            SidebarView(selectedTab: $selectedTab)
        } detail: {
            switch selectedTab {
            case "timeline":
                TimelineView()
            case "models":
                ModelsView()
            case "settings":
                SettingsView()
            default:
                TimelineView()
            }
        }
    }
}

struct SidebarView: View {
    @Binding var selectedTab: String

    var body: some View {
        List {
            Section("工作区") {
                Label("时间线", systemImage: "timeline.selection")
                    .tag("timeline")
                    .onTapGesture { selectedTab = "timeline" }
                Label("模型管理", systemImage: "cube.box")
                    .tag("models")
                    .onTapGesture { selectedTab = "models" }
            }

            Section("设置") {
                Label("API Keys", systemImage: "key")
                    .tag("settings")
                    .onTapGesture { selectedTab = "settings" }
            }
        }
        .listStyle(.sidebar)
    }
}

struct TimelineView: View {
    var body: some View {
        VStack {
            Text("Pool - 时间线")
                .font(.largeTitle)
            Text("P0 Timeline 层")
                .foregroundColor(.secondary)
            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

struct ModelsView: View {
    var body: some View {
        Text("模型管理")
    }
}

struct SettingsView: View {
    var body: some View {
        Text("API Keys 管理")
    }
}

// Note: #Preview macro removed for SPM compatibility.
// Preview works in Xcode. For SPM, use: ContentView()
// as the root view in the app's entry point.
