import SwiftUI

/// Batch Task View
/// Displays and manages batch processing tasks
struct BatchTaskView: View {
    @State private var tasks: [BatchTask] = []
    @State private var showingAddTask = false
    @State private var selectedTasks: Set<String> = []
    @State private var filterStatus: TaskStatusFilter = .all

    enum TaskStatusFilter: String, CaseIterable {
        case all = "All"
        case pending = "Pending"
        case running = "Running"
        case completed = "Completed"
        case failed = "Failed"
    }

    var body: some View {
        VStack(spacing: 0) {
            // Toolbar
            BatchTaskToolbar(
                filterStatus: $filterStatus,
                onAddTask: { showingAddTask = true },
                onCancelSelected: cancelSelectedTasks,
                onRetryFailed: retryFailedTasks,
                onClearCompleted: clearCompletedTasks
            )

            Divider()

            // Task List
            if filteredTasks.isEmpty {
                EmptyBatchStateView()
            } else {
                List(selection: $selectedTasks) {
                    ForEach(filteredTasks) { task in
                        BatchTaskRowView(task: task)
                            .tag(task.id)
                            .contextMenu {
                                Button(action: { retryTask(task) }) {
                                    Label("Retry", systemImage: "arrow.clockwise")
                                }
                                .disabled(task.status != .failed)

                                Button(action: { cancelTask(task) }) {
                                    Label("Cancel", systemImage: "xmark.circle")
                                }
                                .disabled(task.status == .completed || task.status == .cancelled)

                                Divider()

                                Button(role: .destructive, action: { deleteTask(task) }) {
                                    Label("Delete", systemImage: "trash")
                                }
                            }
                    }
                }
            }
        }
        .navigationTitle("Batch Tasks")
        .sheet(isPresented: $showingAddTask) {
            AddBatchTaskView { task in
                addTask(task)
            }
        }
        .onAppear {
            loadTasks()
        }
    }

    // MARK: - Computed Properties

    private var filteredTasks: [BatchTask] {
        switch filterStatus {
        case .all:
            return tasks
        case .pending:
            return tasks.filter { $0.status == .pending }
        case .running:
            return tasks.filter { $0.status == .running }
        case .completed:
            return tasks.filter { $0.status == .completed }
        case .failed:
            return tasks.filter { $0.status == .failed }
        }
    }

    // MARK: - Actions

    private func loadTasks() {
        // Load tasks from CoreBridge FFI
        // For now, use sample data
        tasks = generateSampleTasks()
    }

    private func addTask(_ task: BatchTask) {
        tasks.append(task)
        // In production, call FFI to add task to queue
    }

    private func retryTask(_ task: BatchTask) {
        if let index = tasks.firstIndex(where: { $0.id == task.id }) {
            tasks[index].status = .pending
            tasks[index].error = nil
            tasks[index].progress = 0
        }
    }

    private func cancelTask(_ task: BatchTask) {
        if let index = tasks.firstIndex(where: { $0.id == task.id }) {
            tasks[index].status = .cancelled
        }
    }

    private func deleteTask(_ task: BatchTask) {
        tasks.removeAll { $0.id == task.id }
    }

    private func cancelSelectedTasks() {
        for taskId in selectedTasks {
            if let index = tasks.firstIndex(where: { $0.id == taskId }) {
                tasks[index].status = .cancelled
            }
        }
        selectedTasks.removeAll()
    }

    private func retryFailedTasks() {
        for index in tasks.indices where tasks[index].status == .failed {
            tasks[index].status = .pending
            tasks[index].error = nil
            tasks[index].progress = 0
        }
    }

    private func clearCompletedTasks() {
        tasks.removeAll { $0.status == .completed || $0.status == .cancelled }
    }

    // MARK: - Sample Data

    private func generateSampleTasks() -> [BatchTask] {
        [
            BatchTask(id: "1", name: "Generate Landscape", type: .textToImage, status: .completed, progress: 100),
            BatchTask(id: "2", name: "Upscale Portrait", type: .upscale, status: .running, progress: 65),
            BatchTask(id: "3", name: "Style Transfer", type: .styleTransfer, status: .pending, progress: 0),
            BatchTask(id: "4", name: "Batch Generate", type: .textToImage, status: .failed, progress: 30, error: "Connection timeout"),
        ]
    }
}

// MARK: - Batch Task Model

struct BatchTask: Identifiable, Hashable {
    let id: String
    let name: String
    let type: BatchTaskType
    var status: BatchTaskStatus
    var progress: Float
    var error: String?

    enum BatchTaskType: String {
        case textToImage = "Text-to-Image"
        case upscale = "Upscale"
        case styleTransfer = "Style Transfer"
        case inpainting = "Inpainting"
        case videoGeneration = "Video"
        case export = "Export"
    }

    enum BatchTaskStatus: String {
        case pending = "Pending"
        case running = "Running"
        case completed = "Completed"
        case failed = "Failed"
        case cancelled = "Cancelled"
    }
}

// MARK: - Toolbar

struct BatchTaskToolbar: View {
    @Binding var filterStatus: BatchTaskView.TaskStatusFilter
    let onAddTask: () -> Void
    let onCancelSelected: () -> Void
    let onRetryFailed: () -> Void
    let onClearCompleted: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            // Filter
            Picker("Status", selection: $filterStatus) {
                ForEach(BatchTaskView.TaskStatusFilter.allCases, id: \.self) { status in
                    Text(status.rawValue).tag(status)
                }
            }
            .pickerStyle(.segmented)
            .frame(width: 300)

            Spacer()

            // Actions
            Button(action: onRetryFailed) {
                Image(systemName: "arrow.clockwise")
            }
            .help("Retry Failed")

            Button(action: onClearCompleted) {
                Image(systemName: "trash")
            }
            .help("Clear Completed")

            Divider()
                .frame(height: 20)

            Button(action: onAddTask) {
                Label("Add Task", systemImage: "plus")
            }
            .buttonStyle(.borderedProminent)
        }
        .padding(.horizontal)
        .padding(.vertical, 8)
    }
}

// MARK: - Task Row View

struct BatchTaskRowView: View {
    let task: BatchTask

    var body: some View {
        HStack(spacing: 12) {
            // Status Icon
            statusIcon
                .frame(width: 24)

            // Task Info
            VStack(alignment: .leading, spacing: 4) {
                Text(task.name)
                    .font(.headline)

                HStack(spacing: 8) {
                    Text(task.type.rawValue)
                        .font(.caption)
                        .foregroundColor(.secondary)

                    Text("•")
                        .foregroundColor(.secondary)

                    Text(task.status.rawValue)
                        .font(.caption)
                        .foregroundColor(statusColor)
                }
            }

            Spacer()

            // Progress
            if task.status == .running {
                VStack(alignment: .trailing, spacing: 4) {
                    Text("\(Int(task.progress))%")
                        .font(.caption)
                        .foregroundColor(.secondary)

                    ProgressView(value: task.progress, total: 100)
                        .frame(width: 80)
                }
            } else if task.status == .failed, let error = task.error {
                Text(error)
                    .font(.caption)
                    .foregroundColor(.red)
                    .lineLimit(1)
                    .frame(width: 150, alignment: .trailing)
            }
        }
        .padding(.vertical, 4)
    }

    @ViewBuilder
    private var statusIcon: some View {
        switch task.status {
        case .pending:
            Image(systemName: "clock")
                .foregroundColor(.orange)
        case .running:
            ProgressView()
                .scaleEffect(0.7)
        case .completed:
            Image(systemName: "checkmark.circle.fill")
                .foregroundColor(.green)
        case .failed:
            Image(systemName: "xmark.circle.fill")
                .foregroundColor(.red)
        case .cancelled:
            Image(systemName: "xmark.circle")
                .foregroundColor(.secondary)
        }
    }

    private var statusColor: Color {
        switch task.status {
        case .pending: return .orange
        case .running: return .blue
        case .completed: return .green
        case .failed: return .red
        case .cancelled: return .secondary
        }
    }
}

// MARK: - Empty State

struct EmptyBatchStateView: View {
    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: "list.bullet.rectangle")
                .font(.system(size: 48))
                .foregroundColor(.secondary)

            Text("No Batch Tasks")
                .font(.title2)
                .fontWeight(.medium)

            Text("Add tasks to process multiple images at once")
                .font(.body)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding()
    }
}

// MARK: - Add Task View

struct AddBatchTaskView: View {
    @Environment(\.dismiss) private var dismiss
    @State private var taskName = ""
    @State private var taskType: BatchTask.BatchTaskType = .textToImage
    @State private var prompt = ""

    let onAdd: (BatchTask) -> Void

    var body: some View {
        VStack(spacing: 20) {
            Text("Add Batch Task")
                .font(.title2)
                .fontWeight(.semibold)

            Form {
                Section("Task Details") {
                    TextField("Task Name", text: $taskName)

                    Picker("Task Type", selection: $taskType) {
                        Text("Text-to-Image").tag(BatchTask.BatchTaskType.textToImage)
                        Text("Upscale").tag(BatchTask.BatchTaskType.upscale)
                        Text("Style Transfer").tag(BatchTask.BatchTaskType.styleTransfer)
                        Text("Inpainting").tag(BatchTask.BatchTaskType.inpainting)
                        Text("Video Generation").tag(BatchTask.BatchTaskType.videoGeneration)
                    }

                    if taskType == .textToImage || taskType == .styleTransfer {
                        TextField("Prompt", text: $prompt, axis: .vertical)
                            .lineLimit(3...6)
                    }
                }
            }
            .formStyle(.grouped)

            HStack {
                Button("Cancel") {
                    dismiss()
                }
                .keyboardShortcut(.cancelAction)

                Spacer()

                Button("Add") {
                    let task = BatchTask(
                        id: UUID().uuidString,
                        name: taskName.isEmpty ? taskType.rawValue : taskName,
                        type: taskType,
                        status: .pending,
                        progress: 0
                    )
                    onAdd(task)
                    dismiss()
                }
                .keyboardShortcut(.defaultAction)
                .buttonStyle(.borderedProminent)
                .disabled(taskType == .textToImage && prompt.isEmpty)
            }
            .padding()
        }
        .frame(width: 450, height: 350)
    }
}

// MARK: - Preview

#Preview {
    BatchTaskView()
        .frame(width: 700, height: 500)
}
