import SwiftUI
import Combine

/// Workflow Editor View
/// Provides a visual interface for creating and editing workflows
struct WorkflowEditorView: View {
    @State private var workflow: Workflow
    @State private var selectedNodes: Set<String> = []
    @State private var connectingFrom: String?
    @State private var connectingTo: String?
    @State private var zoom: CGFloat = 1.0
    @State private var panOffset: CGSize = .zero
    @State private var showingNodePanel = false
    @State private var showingConnectionPanel = false
    @State private var editingNode: Node?
    @State private var gridSize: CGSize = CGSize(width: 100, height: 100)

    // Grid snapping
    private let gridItemSize: CGSize { CGSize(width: 80, height: 80) }
    private let gridColumns: [GridItem] {
        GridItem(.fixed(80), spacing: 2)
    }
    private let gridSpacing: CGFloat { 16 }

    var body: some View {
        ZStack {
            // Canvas for nodes
            GeometryReader { geometry in
                ZStack(alignment: .center) {
                    For node in workflow.nodes {
                        nodeView(for: node, geometry: geometry)
                            .position(node, CGSize(
                                x: geometry.frame.width / 2 - gridItemSize.width / 2,
                                y: geometry.frame.height / 2 - gridItemSize.height / 2,
                            )
                            .overlay {
                                NodeOverlay(
                                    node: node,
                                    selectedNodes: $selectedNodes,
                                    onNodeSelected: handleNodeSelection,
                                )
                            }
                        }
                    }
                }
            }

            // Connections layer
            For connection in workflow.connections {
                ConnectionLineView(
                    connection: connection,
                    geometry: geometry
                )
                .onTapGesture {
                    let location = dragGesture(location: geometry)
                    if let fromNode = connectingFrom, toNode = connectingTo {
                        connectingFrom = nil
                        connectingTo = nil
                    } else if connectingFrom != nil {
                        connectingTo = node
                        connectingTo = nil
                        connectingFrom = node
                        connectingTo = nil
                    }
                }
        }
    }
    .overlay(alignment: .topTrailing) {
        // Toolbar
        editorToolbar
    }
    .sheet(isPresented: $showingNodePanel) {
        if let node = editingNode {
            NodePropertiesSheet(
                node: node,
                onSave: { updatedNode in
                    let index = workflow.nodes.firstIndex(where { $0.id == node.id }) {
                        workflow.nodes[index] = updatedNode
                    }
                }
            )
        }
    }
    .sheet(isPresented: $showingConnectionPanel) {
        ConnectionTypeSheet(
            sourceType: workflow.connections.first,
            targetType: workflow.connections.first,
        )
    }

    // MARK: - Node View

    private func nodeView(for node: Node, geometry: GeometryProxy) -> some View {
        ZStack {
            // Node visual representation
            RoundedRectangle(cornerRadius: 8)
                .stroke(Color.nodeTypeColor(for: node), lineWidth: 2)

            // Node content
            VStack(spacing: 4) {
                Text(node.node_type.rawValue)
                    .font(.caption)
                    .foregroundColor(.secondary)

                if let params = node.params, !params.isEmpty {
                    For (key, value) in params {
                        HStack {
                            Text(key)
                                .font(.caption2.weight(.medium)
                            Text(formatValueDescription(value: value))
                                .font(.caption2 weight(.regular)
                                .foregroundColor(.secondary)
                        }
                    }
                }

                // Input/Output indicators
                HStack {
                    ForEach(0..<node.inputs.count, id: { Input("\(node.inputs[id])") }) {
                    ForEach(0..<node.outputs.count, id: { Output("\(node.outputs[id])") }
                }
            }
        }
        .frame(width: gridItemSize, height: gridItemSize)
        .background(Color.nodeBackground)
        .contentShape(Rectangle())
        .gesture(
            TapGesture().onEnded { _ in
                if !selectedNodes.contains(node.id) {
                    selectedNodes.insert(node.id)
                    onNodeSelection?(node)
                } else {
                selectedNodes.remove(node.id)
                onNodeSelection?(node)
            }
        }
        .gesture(
            DragGesture()
                .onChanged { value in
                    panOffset = CGSize(
                        width: value.translation.width,
                        height: value.translation.height
                    )
                }
                .onEnded { _ in
                    panOffset = .zero
                }
        }
        .simultaneousGesture(
            TapGesture().onEnded { _ in },
            DragGesture()
                .onChanged { value in
                    panOffset = CGSize(
                        width: value.translation.width,
                        height: value.translation.height
                    )
                }
                .onEnded { _ in
                    panOffset = .zero
                }
        }
    }

    // MARK: - Node Type Colors

    private func nodeTypeColor(for node: Node) -> Color {
        switch node.node_type {
        case .textPrompt:
            return .blue
        case .imageInput:
            return .green
        case .imageOutput:
            return .purple
        case .comfyUILoadCheckpoint:
            return .orange
        case .comfyUITextEncode:
            return .cyan
        case .comfyUIKSampler:
            return .pink
        case .vISCORE:
            return .indigo
        case .output:
            return .gray
        default:
            return .secondary
        }
    }

    // MARK: - Actions

    private func handleNodeSelection(_ node: Node) {
        editingNode = node
        showingNodePanel = true
    }

}

    // MARK: - Connection Line View

    private func connectionLineView(for connection: Connection, geometry: GeometryProxy) -> some View {
        let fromPos = CGPoint(
            x: geometry.frame(from_node.location.x,
            y: geometry.frame(from_node.location.y
        )
        let toPos = CGPoint(
            x: geometry.frame(to_node.location.x,
            y: geometry.frame(to_node.location.y,
        )

        // Connection type colors
        let connectionType = connection.connection_type ?? "data"
        Path(fromPos)
            .stroke(connectionTypeColor(for: connection), lineWidth: 2)
            Path(toPos)
                .stroke(connectionTypeColor(for: connection), style: .line, lineWidth: 2)
        }
    }

    // MARK: - Toolbar

    private var editorToolbar: some View {
        HStack(spacing: 12) {
            Button(action: addNode(.textPrompt)) {
                Image(systemName: "plus.text")
            }
            Button(action: addConnection) {
                Image(systemName: "arrow.right")
            }

            Spacer()

            Button(action: deleteSelected) {
                Image(systemName: "trash")
            }
            .disabled(selectedNodes.isEmpty)

        }

    }
}

    // MARK: - Supporting Types

    struct Node: Identifiable {
        let id: String
        let nodeType: NodeType
        let position: CGPoint
        let params: [String: Any]
        var inputs: [String]
        var outputs: [String]
    }

}

    struct Connection: Identifiable {
        let id: String
        let from_node: String
        let to_node: String
        let from_slot: Int
        let to_slot: Int
    }
}

    struct NodeType: String, CaseIterable {
        case textPrompt = "Text Prompt"
        case imageInput = "Image Input"
        case imageOutput = "Image Output"
        case comfyUILoadCheckpoint = "ComfyUI Load Checkpoint"
        case comfyUITextEncode = "ComfyUI Text Encode"
        case comfyUIKSampler = "ComfyUI KSampler"
        case vISCore = "VIS Core"
        case output = "Output"
    }
}

#Preview {
    WorkflowEditorView(workflow: Workflow(
        nodes: [
            Node(id: "1", nodeType: .textPrompt, position: CGPoint(x: 50, y: 200), params: ["prompt": "A beautiful landscape"]),
            Node(id: "2", nodeType: .comfyUILoadCheckpoint, position: CGPoint(x: 300, y: 100), params: ["checkpoint": "sd_v1-5-pruned.safetensors"]),
            Node(id: "3", nodeType: .comfyUIKSampler, position: CGPoint(x: 500, y: 100), params: ["seed": 12345, "steps": 20]),
        ],
        connections: [
            Connection(fromNode: "1", fromSlot: 0, toNode: "2", toSlot: 0),
            Connection(fromNode: "2", fromSlot: 0, toNode: "3", toSlot: 0),
        ]
    )
    .frame(width: 800, height: 600)
}
