import SwiftUI

/// Workflow execution status
enum WorkflowStatus {
    case idle
    case running
    case success
    case failed

    var color: Color {
        switch self {
        case .idle: return .gray
        case .running: return .blue
        case .success: return .green
        case .failed: return .red
        }
    }

    var icon: String {
        switch self {
        case .idle: return "circle"
        case .running: return "arrow.trianglehead.clockwise"
        case .success: return "checkmark.circle.fill"
        case .failed: return "xmark.circle.fill"
        }
    }

    var text: String {
        switch self {
        case .idle: return "Ready"
        case .running: return "Running"
        case .success: return "Completed"
        case .failed: return "Failed"
        }
    }
}

/// Node execution status
struct NodeExecutionStatus: Identifiable {
    let id: String
    let name: String
    let status: WorkflowStatus
    let progress: Float
    let message: String?
    let startTime: Date?
    let endTime: Date?

    var duration: TimeInterval? {
        guard let start = startTime else { return nil }
        let end = endTime ?? Date()
        return end.timeIntervalSince(start)
    }
}

/// Workflow Progress View
/// Displays real-time progress of workflow execution
struct WorkflowProgressView: View {
    @State private var workflowStatus: WorkflowStatus = .idle
    @State private var overallProgress: Float = 0.0
    @State private var currentNode: String?
    @State private var currentNodeProgress: Float = 0.0
    @State private var errorMessage: String?
    @State private var nodeStatuses: [NodeExecutionStatus] = []
    @State private var executionStartTime: Date?
    @State private var isExpanded: Bool = true

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            // Header
            HStack {
                Image(systemName: workflowStatus.icon)
                    .foregroundColor(workflowStatus.color)
                    .font(.title2)

                VStack(alignment: .leading, spacing: 2) {
                    Text("Workflow Execution")
                        .font(.headline)
                    Text(statusText)
                        .font(.caption)
                        .foregroundColor(.secondary)
                }

                Spacer()

                if workflowStatus == .running {
                    ProgressView()
                        .scaleEffect(0.7)
                }

                Button(action: { isExpanded.toggle() }) {
                    Image(systemName: isExpanded ? "chevron.down" : "chevron.right")
                        .foregroundColor(.secondary)
                }
                .buttonStyle(.plain)
            }

            if isExpanded {
                Divider()

                // Overall Progress
                VStack(alignment: .leading, spacing: 6) {
                    HStack {
                        Text("Overall Progress")
                            .font(.subheadline)
                            .foregroundColor(.secondary)
                        Spacer()
                        Text("\(Int(overallProgress * 100))%")
                            .font(.subheadline)
                            .fontWeight(.medium)
                    }

                    ProgressView(value: overallProgress)
                        .progressViewStyle(.linear)
                        .tint(workflowStatus.color)
                }

                // Current Node
                if let node = currentNode {
                    VStack(alignment: .leading, spacing: 6) {
                        HStack {
                            Text("Current Node:")
                                .font(.caption)
                                .foregroundColor(.secondary)
                            Text(node)
                                .font(.caption)
                                .fontWeight(.medium)
                        }

                        ProgressView(value: currentNodeProgress)
                            .progressViewStyle(.linear)
                            .tint(.blue)
                    }
                }

                // Execution Time
                if let startTime = executionStartTime {
                    HStack {
                        Image(systemName: "clock")
                            .foregroundColor(.secondary)
                            .font(.caption)
                        Text("Elapsed: \(elapsedTime)")
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }
                }

                // Node List
                if !nodeStatuses.isEmpty {
                    Divider()

                    Text("Node Details")
                        .font(.caption)
                        .foregroundColor(.secondary)

                    ScrollView {
                        LazyVStack(alignment: .leading, spacing: 4) {
                            ForEach(nodeStatuses) { nodeStatus in
                                NodeStatusRow(status: nodeStatus)
                            }
                        }
                    }
                    .frame(maxHeight: 150)
                }

                // Error Message
                if let error = errorMessage {
                    Divider()

                    HStack(alignment: .top, spacing: 8) {
                        Image(systemName: "exclamationmark.triangle.fill")
                            .foregroundColor(.red)
                        Text(error)
                            .font(.caption)
                            .foregroundColor(.red)
                    }
                    .padding(8)
                    .background(Color.red.opacity(0.1))
                    .cornerRadius(6)
                }

                // Action Buttons
                HStack {
                    if workflowStatus == .idle {
                        Button("Start Execution") {
                            startExecution()
                        }
                        .buttonStyle(.borderedProminent)
                    } else if workflowStatus == .running {
                        Button("Cancel") {
                            cancelExecution()
                        }
                        .buttonStyle(.bordered)
                        .foregroundColor(.red)
                    } else {
                        Button("Reset") {
                            resetExecution()
                        }
                        .buttonStyle(.bordered)
                    }
                }
            }
        }
        .padding()
        .background(Color(NSColor.controlBackgroundColor))
        .cornerRadius(10)
    }

    // MARK: - Computed Properties

    private var statusText: String {
        switch workflowStatus {
        case .idle:
            return "Ready to execute"
        case .running:
            if let node = currentNode {
                return "Executing: \(node)"
            }
            return "Processing..."
        case .success:
            return "Execution completed successfully"
        case .failed:
            return "Execution failed"
        }
    }

    private var elapsedTime: String {
        guard let startTime = executionStartTime else { return "0s" }
        let elapsed = Date().timeIntervalSince(startTime)
        return formatDuration(elapsed)
    }

    // MARK: - Helper Functions

    private func formatDuration(_ seconds: TimeInterval) -> String {
        if seconds < 60 {
            return String(format: "%.1fs", seconds)
        } else {
            let minutes = Int(seconds) / 60
            let secs = Int(seconds) % 60
            return String(format: "%dm %ds", minutes, secs)
        }
    }

    // MARK: - Actions

    private func startExecution() {
        workflowStatus = .running
        overallProgress = 0.0
        currentNode = nil
        currentNodeProgress = 0.0
        errorMessage = nil
        nodeStatuses = []
        executionStartTime = Date()

        // Simulate execution (will be replaced with actual FFI calls)
        simulateExecution()
    }

    private func cancelExecution() {
        workflowStatus = .failed
        errorMessage = "Execution cancelled by user"
    }

    private func resetExecution() {
        workflowStatus = .idle
        overallProgress = 0.0
        currentNode = nil
        currentNodeProgress = 0.0
        errorMessage = nil
        nodeStatuses = []
        executionStartTime = nil
    }

    // MARK: - Simulation (to be replaced with FFI)

    private func simulateExecution() {
        let nodes = ["Load Checkpoint", "Empty Latent", "Text Encode", "KSampler", "VAE Decode", "Save Image"]

        for (index, node) in nodes.enumerated() {
            DispatchQueue.main.asyncAfter(deadline: .now() + Double(index) * 1.5) {
                guard self.workflowStatus == .running else { return }

                self.currentNode = node
                self.currentNodeProgress = 0.0

                // Add node to status list
                let nodeStatus = NodeExecutionStatus(
                    id: node,
                    name: node,
                    status: .running,
                    progress: 0.0,
                    message: nil,
                    startTime: Date(),
                    endTime: nil
                )
                self.nodeStatuses.append(nodeStatus)

                // Simulate node progress
                self.simulateNodeProgress(node: node, nodeIndex: index, totalNodes: nodes.count)
            }
        }
    }

    private func simulateNodeProgress(node: String, nodeIndex: Int, totalNodes: Int) {
        for step in 0...10 {
            DispatchQueue.main.asyncAfter(deadline: .now() + Double(step) * 0.12) {
                guard self.workflowStatus == .running else { return }

                let nodeProgress = Float(step) / 10.0
                self.currentNodeProgress = nodeProgress

                // Update node status
                if let idx = self.nodeStatuses.firstIndex(where: { $0.name == node }) {
                    let oldStatus = self.nodeStatuses[idx]
                    self.nodeStatuses[idx] = NodeExecutionStatus(
                        id: oldStatus.id,
                        name: oldStatus.name,
                        status: step == 10 ? .success : .running,
                        progress: nodeProgress,
                        message: step == 10 ? "Completed" : "Processing...",
                        startTime: oldStatus.startTime,
                        endTime: step == 10 ? Date() : nil
                    )
                }

                // Update overall progress
                let baseProgress = Float(nodeIndex) / Float(totalNodes)
                let nodeContribution = nodeProgress / Float(totalNodes)
                self.overallProgress = baseProgress + nodeContribution

                // Check if done
                if nodeIndex == totalNodes - 1 && step == 10 {
                    self.currentNode = nil
                    self.workflowStatus = .success
                }
            }
        }
    }
}

// MARK: - Node Status Row

struct NodeStatusRow: View {
    let status: NodeExecutionStatus

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: status.status.icon)
                .foregroundColor(status.status.color)
                .frame(width: 16)

            Text(status.name)
                .font(.caption)
                .lineLimit(1)

            Spacer()

            if let duration = status.duration {
                Text(String(format: "%.1fs", duration))
                    .font(.caption2)
                    .foregroundColor(.secondary)
            }

            ProgressView(value: status.progress)
                .progressViewStyle(.linear)
                .frame(width: 60)
                .tint(status.status.color)
        }
        .padding(.vertical, 2)
    }
}

// MARK: - Preview

#Preview {
    VStack {
        WorkflowProgressView()
            .frame(width: 400)

        Spacer()
    }
    .padding()
}
