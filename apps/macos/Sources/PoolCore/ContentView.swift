import SwiftUI

/// Main content view for Pool application
/// Provides the primary interface with sidebar navigation and detail views
public struct ContentView: View {
    @State private var selectedTab = "timeline"
    @State private var selectedProject: ProjectItem?
    @State private var searchText = ""

    public init() {}

    public var body: some View {
        NavigationSplitView {
            SidebarView(
                selectedTab: $selectedTab,
                selectedProject: $selectedProject,
                searchText: $searchText
            )
            .frame(minWidth: 220)
        } detail: {
            DetailView(
                selectedTab: selectedTab,
                selectedProject: selectedProject,
                searchText: searchText
            )
        }
        .frame(minWidth: 900, minHeight: 600)
    }
}

// MARK: - Project Item Model

struct ProjectItem: Identifiable, Hashable {
    let id = UUID()
    let name: String
    let shots: Int
    let status: String
    let lastModified: Date

    static func == (lhs: ProjectItem, rhs: ProjectItem) -> Bool {
        lhs.id == rhs.id
    }

    func hash(into hasher: inout Hasher) {
        hasher.combine(id)
    }
}

// MARK: - Sidebar View

struct SidebarView: View {
    @Binding var selectedTab: String
    @Binding var selectedProject: ProjectItem?
    @Binding var searchText: String

    // Sample projects for demonstration
    private let sampleProjects: [ProjectItem] = [
        ProjectItem(name: "Product Demo", shots: 12, status: "In Progress", lastModified: Date()),
        ProjectItem(name: "Brand Video", shots: 8, status: "Draft", lastModified: Date().addingTimeInterval(-86400)),
        ProjectItem(name: "Tutorial Series", shots: 24, status: "Completed", lastModified: Date().addingTimeInterval(-172800))
    ]

    var body: some View {
        List {
            // Quick Actions Section
            Section {
                Button(action: { }) {
                    Label("New Project", systemImage: "plus.circle")
                }
                .buttonStyle(.plain)

                Button(action: { }) {
                    Label("Import Media", systemImage: "square.and.arrow.down")
                }
                .buttonStyle(.plain)
            } header: {
                Text("Quick Actions")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }

            // Workspace Section
            Section {
                NavigationLink(value: "timeline") {
                    Label("Timeline", systemImage: "timeline.selection")
                        .tag("timeline")
                }

                NavigationLink(value: "node_editor") {
                    Label("Node Editor", systemImage: "square.grid.3x3")
                        .tag("node_editor")
                }

                NavigationLink(value: "workflow") {
                    Label("Workflow", systemImage: "arrow.trianglehead.clockwise")
                        .tag("workflow")
                }

                NavigationLink(value: "results") {
                    Label("Results", systemImage: "photo.on.rectangle")
                        .tag("results")
                }

                NavigationLink(value: "models") {
                    Label("Model Manager", systemImage: "cube.box")
                        .tag("models")
                }
            } header: {
                Text("Workspace")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }

            // Projects Section
            Section {
                ForEach(sampleProjects) { project in
                    ProjectRowView(project: project)
                        .tag(project)
                        .onTapGesture {
                            selectedProject = project
                            selectedTab = "timeline"
                        }
                }
            } header: {
                HStack {
                    Text("Projects")
                        .font(.caption)
                        .foregroundColor(.secondary)
                    Spacer()
                    Button(action: { }) {
                        Image(systemName: "plus")
                            .font(.caption)
                    }
                    .buttonStyle(.plain)
                }
            }

            // Settings Section
            Section {
                NavigationLink(value: "settings") {
                    Label("API Keys", systemImage: "key")
                        .tag("settings")
                }

                NavigationLink(value: "preferences") {
                    Label("Preferences", systemImage: "gearshape")
                        .tag("preferences")
                }
            } header: {
                Text("Settings")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
        }
        .searchable(text: $searchText, prompt: "Search projects...")
        .listStyle(.sidebar)
        .navigationTitle("Pool")
    }
}

// MARK: - Project Row View

struct ProjectRowView: View {
    let project: ProjectItem

    var body: some View {
        HStack(spacing: 12) {
            // Status indicator
            Circle()
                .fill(statusColor)
                .frame(width: 8, height: 8)

            VStack(alignment: .leading, spacing: 2) {
                Text(project.name)
                    .font(.body)
                    .lineLimit(1)

                HStack(spacing: 4) {
                    Text("\(project.shots) shots")
                        .font(.caption)
                        .foregroundColor(.secondary)

                    Text("-")
                        .foregroundColor(.secondary)

                    Text(project.status)
                        .font(.caption)
                        .foregroundColor(statusColor)
                }
            }
        }
        .padding(.vertical, 2)
    }

    private var statusColor: Color {
        switch project.status {
        case "Completed":
            return .green
        case "In Progress":
            return .blue
        case "Draft":
            return .orange
        default:
            return .gray
        }
    }
}

// MARK: - Detail View

struct DetailView: View {
    let selectedTab: String
    let selectedProject: ProjectItem?
    let searchText: String

    var body: some View {
        Group {
            switch selectedTab {
            case "timeline":
                TimelineView(project: selectedProject)
            case "node_editor":
                NodeEditorView()
            case "workflow":
                WorkflowProgressView()
            case "results":
                ResultsGalleryView()
            case "models":
                ModelsView()
            case "settings":
                SettingsView()
            case "preferences":
                PreferencesView()
            default:
                EmptyView()
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

// MARK: - Preview Support

// Note: #Preview macro removed for SPM compatibility.
// Preview works in Xcode. For SPM, use: ContentView()
// as the root view in the app's entry point.
