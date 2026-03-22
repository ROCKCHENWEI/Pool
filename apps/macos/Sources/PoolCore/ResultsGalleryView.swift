import SwiftUI

/// Generated result item
struct GeneratedResult: Identifiable, Hashable {
    let id: UUID
    let type: ResultType
    let thumbnailPath: String?
    let filePath: String
    let createdAt: Date
    let prompt: String?
    let metadata: [String: String]?

    enum ResultType {
        case image
        case video

        var icon: String {
            switch self {
            case .image: return "photo"
            case .video: return "video"
            }
        }

        var extension: String {
            switch self {
            case .image: return "png"
            case .video: return "mp4"
            }
        }
    }
}

/// Results Gallery View
/// Displays generated images and videos in a grid layout
struct ResultsGalleryView: View {
    @State private var results: [GeneratedResult] = []
    @State private var selectedResult: GeneratedResult?
    @State private var showingDetailView = false
    @State private var filterType: ResultFilter = .all
    @State private var searchText = ""
    @State private var sortBy: SortOption = .dateDescending
    @State private var showingExporter = false

    enum ResultFilter: String, CaseIterable {
        case all = "All"
        case images = "Images"
        case videos = "Videos"
    }

    enum SortOption: String, CaseIterable {
        case dateDescending = "Newest First"
        case dateAscending = "Oldest First"
        case nameAscending = "Name A-Z"
    }

    var body: some View {
        VStack(spacing: 0) {
            // Toolbar
            ResultsToolbar(
                filterType: $filterType,
                sortBy: $sortBy,
                searchText: $searchText,
                onRefresh: loadResults,
                onExport: { showingExporter = true }
            )

            Divider()

            // Content
            if filteredResults.isEmpty {
                EmptyStateView()
            } else {
                ScrollView {
                    LazyVGrid(columns: gridColumns, spacing: 16) {
                        ForEach(filteredResults) { result in
                            ResultThumbnailView(result: result)
                                .onTapGesture {
                                    selectedResult = result
                                    showingDetailView = true
                                }
                                .contextMenu {
                                    Button(action: { exportResult(result) }) {
                                        Label("Export", systemImage: "square.and.arrow.up")
                                    }
                                    Button(action: { revealInFinder(result) }) {
                                        Label("Reveal in Finder", systemImage: "folder")
                                    }
                                    Divider()
                                    Button(role: .destructive, action: { deleteResult(result) }) {
                                        Label("Delete", systemImage: "trash")
                                    }
                                }
                        }
                    }
                    .padding()
                }
            }
        }
        .navigationTitle("Generated Results")
        .sheet(isPresented: $showingDetailView) {
            if let result = selectedResult {
                ResultDetailView(result: result)
            }
        }
        .sheet(isPresented: $showingExporter) {
            ExportSheetView(results: selectedResultsForExport)
        }
        .onAppear {
            loadResults()
        }
    }

    // MARK: - Computed Properties

    private var gridColumns: [GridItem] {
        Array(repeating: GridItem(.adaptive(minimum: 150, maximum: 200), spacing: 16), count: 1)
    }

    private var filteredResults: [GeneratedResult] {
        var filtered = results

        // Filter by type
        switch filterType {
        case .images:
            filtered = filtered.filter { $0.type == .image }
        case .videos:
            filtered = filtered.filter { $0.type == .video }
        case .all:
            break
        }

        // Filter by search
        if !searchText.isEmpty {
            filtered = filtered.filter {
                $0.prompt?.localizedCaseContains(searchText) ?? false ||
                $0.filePath.localizedCaseContains(searchText)
            }
        }

        // Sort
        switch sortBy {
        case .dateDescending:
            filtered.sort { $0.createdAt > $1.createdAt }
        case .dateAscending:
            filtered.sort { $0.createdAt < $1.createdAt }
        case .nameAscending:
            filtered.sort { $0.filePath < $1.filePath }
        }

        return filtered
    }

    private var selectedResultsForExport: [GeneratedResult] {
        if let selected = selectedResult {
            return [selected]
        }
        return filteredResults
    }

    // MARK: - Actions

    private func loadResults() {
        // Load results from storage
        // In production, this would call into Rust FFI
        results = generateSampleResults()
    }

    private func exportResult(_ result: GeneratedResult) {
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.png, .mpeg4Movie]
        panel.nameFieldStringValue = URL(fileURLWithPath: result.filePath).lastPathComponent

        if panel.runModal() == .OK, let url = panel.url {
            try? FileManager.default.copyItem(at: URL(fileURLWithPath: result.filePath), to: url)
        }
    }

    private func revealInFinder(_ result: GeneratedResult) {
        let url = URL(fileURLWithPath: result.filePath)
        NSWorkspace.shared.activateFileViewerSelecting([url])
    }

    private func deleteResult(_ result: GeneratedResult) {
        results.removeAll { $0.id == result.id }
        // In production, also delete from filesystem
    }

    // MARK: - Sample Data

    private func generateSampleResults() -> [GeneratedResult] {
        (0..<12).map { i in
            GeneratedResult(
                id: UUID(),
                type: i % 3 == 0 ? .video : .image,
                thumbnailPath: nil,
                filePath: "/tmp/result_\(i).\(i % 3 == 0 ? "mp4" : "png")",
                createdAt: Date().addingTimeInterval(-Double(i * 3600)),
                prompt: i % 2 == 0 ? "A beautiful sunset over mountains" : "Abstract art with vibrant colors",
                metadata: ["model": "SDXL", "steps": "20", "seed": "\(12345 + i)"]
            )
        }
    }
}

// MARK: - Toolbar

struct ResultsToolbar: View {
    @Binding var filterType: ResultsGalleryView.ResultFilter
    @Binding var sortBy: ResultsGalleryView.SortOption
    @Binding var searchText: String
    let onRefresh: () -> Void
    let onExport: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            // Search
            HStack {
                Image(systemName: "magnifyingglass")
                    .foregroundColor(.secondary)
                TextField("Search results...", text: $searchText)
                    .textFieldStyle(.plain)
                if !searchText.isEmpty {
                    Button(action: { searchText = "" }) {
                        Image(systemName: "xmark.circle.fill")
                            .foregroundColor(.secondary)
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(Color(nsColor: .controlBackgroundColor))
            .cornerRadius(6)
            .frame(width: 200)

            Spacer()

            // Filter
            Picker("Filter", selection: $filterType) {
                ForEach(ResultsGalleryView.ResultFilter.allCases, id: \.self) { filter in
                    Text(filter.rawValue).tag(filter)
                }
            }
            .pickerStyle(.segmented)
            .frame(width: 200)

            // Sort
            Menu {
                ForEach(ResultsGalleryView.SortOption.allCases, id: \.self) { option in
                    Button(option.rawValue) {
                        sortBy = option
                    }
                }
            } label: {
                HStack(spacing: 4) {
                    Image(systemName: "arrow.up.arrow.down")
                    Text(sortBy.rawValue)
                }
            }

            Divider()
                .frame(height: 20)

            // Actions
            Button(action: onRefresh) {
                Image(systemName: "arrow.clockwise")
            }
            .help("Refresh")

            Button(action: onExport) {
                Image(systemName: "square.and.arrow.up")
            }
            .help("Export")
        }
        .padding(.horizontal)
        .padding(.vertical, 8)
    }
}

// MARK: - Thumbnail View

struct ResultThumbnailView: View {
    let result: GeneratedResult

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            // Thumbnail
            ZStack {
                thumbnailImage
                    .aspectRatio(1, contentMode: .fill)
                    .clipped()
                    .cornerRadius(8)

                // Type badge
                HStack(spacing: 4) {
                    Image(systemName: result.type == .video ? "video.fill" : "photo.fill")
                        .font(.caption2)
                    Text(result.type == .video ? "Video" : "Image")
                        .font(.caption2)
                }
                .padding(.horizontal, 6)
                .padding(.vertical, 2)
                .background(.ultraThinMaterial)
                .cornerRadius(4)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomLeading)
                .padding(8)
            }
            .frame(height: 150)
            .background(Color(nsColor: .windowBackgroundColor))

            // Info
            VStack(alignment: .leading, spacing: 4) {
                if let prompt = result.prompt {
                    Text(prompt)
                        .font(.caption)
                        .lineLimit(2)
                        .foregroundColor(.primary)
                }

                Text(result.createdAt, style: .relative)
                    .font(.caption2)
                    .foregroundColor(.secondary)
            }
            .padding(.horizontal, 4)
        }
        .background(Color.clear)
    }

    @ViewBuilder
    private var thumbnailImage: some View {
        if let path = result.thumbnailPath,
           let image = NSImage(contentsOfFile: path) {
            Image(nsImage: image)
                .resizable()
        } else {
            // Placeholder
            Rectangle()
                .fill(
                    LinearGradient(
                        colors: [Color.gray.opacity(0.3), Color.gray.opacity(0.1)],
                        startPoint: .topLeading,
                        endPoint: .bottomTrailing
                    )
                )
                .overlay {
                    Image(systemName: result.type.icon)
                        .font(.largeTitle)
                        .foregroundColor(.white.opacity(0.5))
                }
        }
    }
}

// MARK: - Empty State

struct EmptyStateView: View {
    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: "photo.on.rectangle.angled")
                .font(.system(size: 48))
                .foregroundColor(.secondary)

            Text("No Results Yet")
                .font(.title2)
                .fontWeight(.medium)

            Text("Generated images and videos will appear here")
                .font(.body)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)

            Button("Create Your First Generation") {
                // Navigate to workflow
            }
            .buttonStyle(.borderedProminent)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding()
    }
}

// MARK: - Detail View

struct ResultDetailView: View {
    let result: GeneratedResult
    @Environment(\.dismiss) private var dismiss
    @State private var isPlaying = false

    var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack {
                VStack(alignment: .leading, spacing: 4) {
                    Text(result.type == .video ? "Video" : "Image")
                        .font(.caption)
                        .foregroundColor(.secondary)
                    Text(result.createdAt, style: .date)
                        .font(.headline)
                }

                Spacer()

                Button(action: { exportResult() }) {
                    Label("Export", systemImage: "square.and.arrow.up")
                }
                .buttonStyle(.bordered)

                Button(action: { dismiss() }) {
                    Image(systemName: "xmark.circle.fill")
                        .font(.title2)
                        .foregroundColor(.secondary)
                }
                .buttonStyle(.plain)
            }
            .padding()

            Divider()

            // Content
            ScrollView {
                VStack(spacing: 16) {
                    // Preview
                    previewView
                        .frame(maxWidth: 600, maxHeight: 500)
                        .cornerRadius(12)

                    // Prompt
                    if let prompt = result.prompt {
                        VStack(alignment: .leading, spacing: 8) {
                            Text("Prompt")
                                .font(.caption)
                                .foregroundColor(.secondary)
                            Text(prompt)
                                .font(.body)
                                .padding()
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .background(Color(nsColor: .controlBackgroundColor))
                                .cornerRadius(8)
                        }
                    }

                    // Metadata
                    if let metadata = result.metadata, !metadata.isEmpty {
                        VStack(alignment: .leading, spacing: 8) {
                            Text("Generation Settings")
                                .font(.caption)
                                .foregroundColor(.secondary)

                            LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 8) {
                                ForEach(Array(metadata.keys.sorted()), id: \.self) { key in
                                    HStack {
                                        Text(key)
                                            .font(.caption)
                                            .foregroundColor(.secondary)
                                        Spacer()
                                        Text(metadata[key] ?? "")
                                            .font(.caption)
                                            .fontWeight(.medium)
                                    }
                                    .padding(8)
                                    .background(Color(nsColor: .controlBackgroundColor))
                                    .cornerRadius(6)
                                }
                            }
                        }
                    }
                }
                .padding()
            }
        }
        .frame(width: 700, height: 600)
    }

    @ViewBuilder
    private var previewView: some View {
        if result.type == .video {
            ZStack {
                Rectangle()
                    .fill(Color.black)

                if isPlaying {
                    // Video player would go here
                    Text("Video Player")
                        .foregroundColor(.white)
                } else {
                    Image(systemName: "play.circle.fill")
                        .font(.system(size: 64))
                        .foregroundColor(.white)
                        .onTapGesture {
                            isPlaying = true
                        }
                }
            }
        } else {
            // Image preview
            ZStack {
                Rectangle()
                    .fill(Color(nsColor: .windowBackgroundColor))

                if let path = result.thumbnailPath,
                   let image = NSImage(contentsOfFile: path) {
                    Image(nsImage: image)
                        .resizable()
                        .aspectRatio(contentMode: .fit)
                } else {
                    Image(systemName: "photo")
                        .font(.system(size: 64))
                        .foregroundColor(.secondary)
                }
            }
        }
    }

    private func exportResult() {
        // Export logic
    }
}

// MARK: - Export Sheet

struct ExportSheetView: View {
    let results: [GeneratedResult]
    @Environment(\.dismiss) private var dismiss
    @State private var exportFormat: ExportFormat = .original
    @State private var exportPath = "~/Desktop/Pool Exports"

    enum ExportFormat: String, CaseIterable {
        case original = "Original Format"
        case png = "PNG (Images Only)"
        case mp4 = "MP4 (Videos Only)"
    }

    var body: some View {
        VStack(spacing: 20) {
            Text("Export Results")
                .font(.title2)
                .fontWeight(.semibold)

            VStack(alignment: .leading, spacing: 12) {
                Text("Exporting \(results.count) item(s)")
                    .foregroundColor(.secondary)

                Picker("Format", selection: $exportFormat) {
                    ForEach(ExportFormat.allCases, id: \.self) { format in
                        Text(format.rawValue).tag(format)
                    }
                }

                HStack {
                    TextField("Export Path", text: $exportPath)
                    Button("Choose...") {
                        let panel = NSOpenPanel()
                        panel.canChooseDirectories = true
                        panel.canChooseFiles = false
                        if panel.runModal() == .OK, let url = panel.url {
                            exportPath = url.path
                        }
                    }
                }
            }
            .padding()
            .background(Color(nsColor: .controlBackgroundColor))
            .cornerRadius(8)

            HStack {
                Button("Cancel") {
                    dismiss()
                }
                .keyboardShortcut(.cancelAction)

                Spacer()

                Button("Export") {
                    performExport()
                    dismiss()
                }
                .keyboardShortcut(.defaultAction)
                .buttonStyle(.borderedProminent)
            }
        }
        .padding()
        .frame(width: 400)
    }

    private func performExport() {
        // Export logic
    }
}

// MARK: - Preview

#Preview {
    ResultsGalleryView()
        .frame(width: 800, height: 600)
}
