import SwiftUI

/// Node Editor View - Visual node-based workflow editor
/// Allows creating and connecting nodes for AI generation workflows
struct NodeEditorView: View {
    @State private var nodes: [WorkflowNode] = [
        WorkflowNode(
            id: UUID(),
            type: .input,
            name: "Image Input",
            position: CGPoint(x: 100, y: 200)
        ),
        WorkflowNode(
            id: UUID(),
            type: .ai,
            name: "Style Transfer",
            position: CGPoint(x: 350, y: 150)
        ),
        WorkflowNode(
            id: UUID(),
            type: .ai,
            name: "Upscale",
            position: CGPoint(x: 350, y: 300)
        ),
        WorkflowNode(
            id: UUID(),
            type: .output,
            name: "Video Output",
            position: CGPoint(x: 600, y: 225)
        )
    ]

    @State private var connections: [NodeConnection] = [
        NodeConnection(from: UUID(), to: UUID()),
        NodeConnection(from: UUID(), to: UUID()),
        NodeConnection(from: UUID(), to: UUID())
    ]

    @State private var selectedNode: UUID?
    @State private var offset = CGSize.zero
    @State private var showNodeLibrary = false

    var body: some View {
        HStack(spacing: 0) {
            // Node Library Sidebar
            nodeLibrarySidebar
                .frame(width: 200)
                .background(Color(nsColor: .controlBackgroundColor))

            Divider()

            // Main Canvas
            ZStack {
                // Grid background
                NodeEditorGridView()
                    .background(Color(nsColor: .textBackgroundColor))

                // Connections
                ForEach(connections) { connection in
                    ConnectionLine(connection: connection, nodes: nodes)
                        .stroke(Color.accentColor, lineWidth: 2)
                }

                // Nodes
                ForEach($nodes) { $node in
                    NodeView(
                        node: node,
                        isSelected: selectedNode == node.id
                    )
                    .position(node.position)
                    .gesture(
                        DragGesture()
                            .onChanged { value in
                                node.position = CGPoint(
                                    x: node.position.x + value.translation.width - offset.width,
                                    y: node.position.y + value.translation.height - offset.height
                                )
                                offset = value.translation
                            }
                            .onEnded { _ in
                                offset = .zero
                            }
                    )
                    .onTapGesture {
                        selectedNode = node.id
                    }
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)

            // Properties Panel
            if let nodeId = selectedNode,
               let node = nodes.first(where: { $0.id == nodeId }) {
                propertiesPanel(node: node)
                    .frame(width: 250)
                    .background(Color(nsColor: .controlBackgroundColor))
            }
        }
        .navigationTitle("Node Editor")
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button(action: { showNodeLibrary.toggle() }) {
                    Label("Node Library", systemImage: "square.grid.3x3")
                }
            }
            ToolbarItem(placement: .primaryAction) {
                Button(action: { }) {
                    Label("Run Workflow", systemImage: "play.fill")
                }
                .buttonStyle(.borderedProminent)
            }
        }
    }

    // MARK: - Node Library Sidebar

    private var nodeLibrarySidebar: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("Node Library")
                .font(.headline)
                .padding()

            Divider()

            List {
                Section("Input Nodes") {
                    NodeLibraryItem(name: "Image Input", icon: "photo", type: .input)
                    NodeLibraryItem(name: "Video Input", icon: "video", type: .input)
                    NodeLibraryItem(name: "Text Input", icon: "textformat", type: .input)
                }

                Section("AI Nodes") {
                    NodeLibraryItem(name: "Style Transfer", icon: "wand.and.stars", type: .ai)
                    NodeLibraryItem(name: "Image Generation", icon: "paintbrush", type: .ai)
                    NodeLibraryItem(name: "Upscale", icon: "arrow.up.left.and.arrow.down.right", type: .ai)
                    NodeLibraryItem(name: "Video Generation", icon: "video.fill", type: .ai)
                }

                Section("ComfyUI - Basic") {
                    NodeLibraryItem(name: "Load Checkpoint", icon: "externaldrive.badge.icloud", type: .comfyuiLoadCheckpoint)
                    NodeLibraryItem(name: "Text Encode", icon: "textformat", type: .comfyuiTextEncode)
                    NodeLibraryItem(name: "Empty Latent", icon: "rectangle.dashed", type: .comfyuiEmptyLatent)
                    NodeLibraryItem(name: "KSampler", icon: "dial.medium", type: .comfyuiKSampler)
                    NodeLibraryItem(name: "VAE Decode", icon: "photo", type: .comfyuiVAEDecode)
                    NodeLibraryItem(name: "Save Image", icon: "square.and.arrow.down.on.square", type: .comfyuiSaveImage)
                }

                Section("ComfyUI - Advanced") {
                    NodeLibraryItem(name: "CLIP Vision", icon: "eye", type: .comfyuiClipVision)
                    NodeLibraryItem(name: "ControlNet", icon: "slider.horizontal.3", type: .comfyuiControlNet)
                }

                Section("Output Nodes") {
                    NodeLibraryItem(name: "Video Output", icon: "square.and.arrow.up", type: .output)
                    NodeLibraryItem(name: "Image Output", icon: "square.and.arrow.up", type: .output)
                }

                Section("Utility Nodes") {
                    NodeLibraryItem(name: "Merge", icon: "arrow.merge", type: .utility)
                    NodeLibraryItem(name: "Split", icon: "arrow.branch", type: .utility)
                    NodeLibraryItem(name: "Condition", icon: "arrow.triangle.branch", type: .utility)
                }
            }
            .listStyle(.inset)
        }
    }

    // MARK: - Properties Panel

    private func propertiesPanel(node: WorkflowNode) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("Properties")
                .font(.headline)
                .padding()

            Divider()

            Form {
                Section("Node Info") {
                    LabeledContent("Name") {
                        TextField("", text: .constant(node.name))
                    }
                    LabeledContent("Type") {
                        Text(node.type.displayName)
                            .foregroundColor(.secondary)
                    }
                }

                Section("Parameters") {
                    ForEach(node.parameters, id: \.name) { param in
                        ParameterRow(parameter: param)
                    }
                }
            }
            .formStyle(.grouped)

            Spacer()

            // Action buttons
            HStack {
                Button("Delete Node", role: .destructive) {
                    nodes.removeAll { $0.id == node.id }
                    selectedNode = nil
                }
                .buttonStyle(.bordered)
            }
            .padding()
        }
    }
}

// MARK: - Workflow Node Model

struct WorkflowNode: Identifiable {
    let id: UUID
    let type: NodeType
    let name: String
    var position: CGPoint
    var parameters: [NodeParameter] = []

    enum NodeType {
        case input, ai, output, utility
        case comfyuiTextEncode, comfyuiKSampler, comfyuiVAEDecode, comfyuiSaveImage
        case comfyuiLoadCheckpoint, comfyuiEmptyLatent, comfyuiClipVision, comfyuiControlNet

        var displayName: String {
            switch self {
            case .input: return "Input"
            case .ai: return "AI"
            case .output: return "Output"
            case .utility: return "Utility"
            case .comfyuiTextEncode: return "Text Encode"
            case .comfyuiKSampler: return "KSampler"
            case .comfyuiVAEDecode: return "VAE Decode"
            case .comfyuiSaveImage: return "Save Image"
            case .comfyuiLoadCheckpoint: return "Load Checkpoint"
            case .comfyuiEmptyLatent: return "Empty Latent"
            case .comfyuiClipVision: return "CLIP Vision"
            case .comfyuiControlNet: return "ControlNet"
            }
        }

        var color: Color {
            switch self {
            case .input: return .blue
            case .ai: return .purple
            case .output: return .green
            case .utility: return .orange
            // ComfyUI nodes use a teal/cyan color
            case .comfyuiTextEncode: return Color(red: 0.0, green: 0.8, blue: 0.8)
            case .comfyuiKSampler: return Color(red: 0.2, green: 0.6, blue: 0.8)
            case .comfyuiVAEDecode: return Color(red: 0.0, green: 0.7, blue: 0.7)
            case .comfyuiSaveImage: return Color(red: 0.3, green: 0.7, blue: 0.6)
            case .comfyuiLoadCheckpoint: return Color(red: 0.1, green: 0.75, blue: 0.75)
            case .comfyuiEmptyLatent: return Color(red: 0.15, green: 0.65, blue: 0.85)
            case .comfyuiClipVision: return Color(red: 0.05, green: 0.72, blue: 0.78)
            case .comfyuiControlNet: return Color(red: 0.25, green: 0.68, blue: 0.82)
            }
        }

        var isComfyUI: Bool {
            switch self {
            case .comfyuiTextEncode, .comfyuiKSampler, .comfyuiVAEDecode,
                 .comfyuiSaveImage, .comfyuiLoadCheckpoint, .comfyuiEmptyLatent,
                 .comfyuiClipVision, .comfyuiControlNet:
                return true
            default:
                return false
            }
        }

        var iconName: String {
            switch self {
            case .input: return "square.and.arrow.down"
            case .ai: return "brain"
            case .output: return "square.and.arrow.up"
            case .utility: return "gearshape"
            case .comfyuiTextEncode: return "textformat"
            case .comfyuiKSampler: return "dial.medium"
            case .comfyuiVAEDecode: return "photo"
            case .comfyuiSaveImage: return "square.and.arrow.down.on.square"
            case .comfyuiLoadCheckpoint: return "externaldrive.badge.icloud"
            case .comfyuiEmptyLatent: return "rectangle.dashed"
            case .comfyuiClipVision: return "eye"
            case .comfyuiControlNet: return "slider.horizontal.3"
            }
        }
    }
}

struct NodeParameter {
    let name: String
    let type: ParamType
    var value: Any

    enum ParamType {
        case text, number, slider, dropdown
    }
}

// MARK: - Node Connection

struct NodeConnection: Identifiable {
    let id = UUID()
    let from: UUID
    let to: UUID
}

// MARK: - Node View

struct NodeView: View {
    let node: WorkflowNode
    let isSelected: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            // Header
            HStack {
                Image(systemName: "cube")
                    .font(.caption)
                Text(node.name)
                    .font(.caption)
                    .fontWeight(.medium)
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .frame(maxWidth: .infinity)
            .background(node.type.color)

            // Content
            VStack(alignment: .leading, spacing: 4) {
                ForEach(0..<min(3, node.parameters.count), id: \.self) { _ in
                    HStack {
                        Circle()
                            .fill(Color.gray.opacity(0.5))
                            .frame(width: 8, height: 8)
                        Text("Parameter")
                            .font(.caption2)
                            .foregroundColor(.secondary)
                    }
                }

                if node.parameters.isEmpty {
                    Text("No parameters")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                }
            }
            .padding(8)
        }
        .frame(width: 140)
        .background(Color(nsColor: .windowBackgroundColor))
        .cornerRadius(8)
        .shadow(radius: 4)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(isSelected ? Color.accentColor : Color.clear, lineWidth: 2)
        )
    }
}

// MARK: - Connection Line

struct ConnectionLine: Shape {
    let connection: NodeConnection
    let nodes: [WorkflowNode]

    func path(in rect: CGRect) -> Path {
        guard let fromNode = nodes.first(where: { $0.id == connection.from }),
              let toNode = nodes.first(where: { $0.id == connection.to }) else {
            return Path()
        }

        let start = fromNode.position
        let end = toNode.position

        var path = Path()
        path.move(to: start)

        // Bezier curve for smooth connection
        let controlOffset: CGFloat = 100
        let control1 = CGPoint(x: start.x + controlOffset, y: start.y)
        let control2 = CGPoint(x: end.x - controlOffset, y: end.y)

        path.addCurve(to: end, control1: control1, control2: control2)

        return path
    }
}

// MARK: - Node Editor Grid View

struct NodeEditorGridView: View {
    var body: some View {
        GeometryReader { geometry in
            Path { path in
                // Vertical lines
                let step: CGFloat = 30
                var x: CGFloat = step
                while x < geometry.size.width {
                    path.move(to: CGPoint(x: x, y: 0))
                    path.addLine(to: CGPoint(x: x, y: geometry.size.height))
                    x += step
                }

                // Horizontal lines
                var y: CGFloat = step
                while y < geometry.size.height {
                    path.move(to: CGPoint(x: 0, y: y))
                    path.addLine(to: CGPoint(x: geometry.size.width, y: y))
                    y += step
                }
            }
            .stroke(Color.gray.opacity(0.15), lineWidth: 1)
        }
    }
}

// MARK: - Node Library Item

struct NodeLibraryItem: View {
    let name: String
    let icon: String
    let type: WorkflowNode.NodeType

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: icon)
                .frame(width: 20)
                .foregroundColor(type.color)

            Text(name)
                .font(.caption)
        }
        .padding(4)
        .background(Color(nsColor: .windowBackgroundColor))
        .cornerRadius(4)
        .onDrag {
            NSItemProvider(object: name as NSString)
        }
    }
}

// MARK: - Parameter Row

struct ParameterRow: View {
    let parameter: NodeParameter

    var body: some View {
        HStack {
            Text(parameter.name)
                .font(.caption)
            Spacer()

            switch parameter.type {
            case .text:
                Text("Text")
                    .font(.caption2)
                    .foregroundColor(.secondary)
            case .number:
                Text("Number")
                    .font(.caption2)
                    .foregroundColor(.secondary)
            case .slider:
                Slider(value: .constant(0.5), in: 0...1)
                    .frame(width: 80)
            case .dropdown:
                Text("Dropdown")
                    .font(.caption2)
                    .foregroundColor(.secondary)
            }
        }
    }
}

// MARK: - Preview

#Preview {
    NodeEditorView()
        .frame(width: 1000, height: 600)
}
