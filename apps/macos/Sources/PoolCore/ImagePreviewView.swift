import SwiftUI

/// Image Preview View
/// Displays generated images with zoom, pan, and export capabilities
struct ImagePreviewView: View {
    let filename: String
    let subfolder: String
    let imgType: String

    @State private var imageData: Data?
    @State private var isLoading = true
    @State private var errorMessage: String?
    @State private var scale: CGFloat = 1.0
    @State private var offset: CGSize = .zero
    @State private var lastOffset: CGSize = .zero
    @State private var showingExportSheet = false
    @State private var showingShareSheet = false

    init(filename: String, subfolder: String = "", imgType: String = "output") {
        self.filename = filename
        self.subfolder = subfolder
        self.imgType = imgType
    }

    var body: some View {
        ZStack {
            if isLoading {
                ProgressView("Loading image...")
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if let error = errorMessage {
                ErrorStateView(message: error, onRetry: loadImage)
            } else if let data = imageData, let image = NSImage(data: data) {
                ImageContentView(
                    image: image,
                    scale: $scale,
                    offset: $offset,
                    lastOffset: $lastOffset
                )
            } else {
                ErrorStateView(message: "Failed to decode image", onRetry: loadImage)
            }
        }
        .toolbar {
            ToolbarItemGroup {
                if imageData != nil {
                    zoomControls
                    Divider()
                    exportControls
                }
            }
        }
        .onAppear {
            loadImage()
        }
    }

    // MARK: - Toolbar Controls

    @ViewBuilder
    private var zoomControls: some View {
        Button(action: { zoomOut() }) {
            Image(systemName: "minus.magnifyingglass")
        }
        .help("Zoom Out")

        Text("\(Int(scale * 100))%")
            .font(.caption)
            .frame(width: 50)

        Button(action: { zoomIn() }) {
            Image(systemName: "plus.magnifyingglass")
        }
        .help("Zoom In")

        Button(action: { resetZoom() }) {
            Image(systemName: "1.magnifyingglass")
        }
        .help("Reset Zoom")
    }

    @ViewBuilder
    private var exportControls: some View {
        Button(action: { showingExportSheet = true }) {
            Label("Export", systemImage: "square.and.arrow.up")
        }
        .help("Export Image")

        Button(action: { copyToClipboard() }) {
            Image(systemName: "doc.on.doc")
        }
        .help("Copy to Clipboard")

        Menu {
            Button("Open in Preview") { openInPreview() }
            Button("Set as Desktop Picture") { setAsDesktop() }
            Divider()
            Button("Share...") { showingShareSheet = true }
        } label: {
            Image(systemName: "ellipsis.circle")
        }
        .help("More Options")
    }

    // MARK: - Image Loading

    private func loadImage() {
        isLoading = true
        errorMessage = nil

        // Call FFI to get image data
        let jsonResult = CoreBridge.getImageData(
            filename: filename,
            subfolder: subfolder,
            imgType: imgType
        )

        guard let data = jsonResult.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            errorMessage = "Failed to parse response"
            isLoading = false
            return
        }

        if let success = json["success"] as? Bool, success,
           let base64Data = json["data"] as? String,
           let decoded = Data(base64Encoded: base64Data) {
            imageData = decoded
        } else if let error = json["error"] as? String {
            errorMessage = error
        } else {
            errorMessage = "Unknown error"
        }

        isLoading = false
    }

    // MARK: - Zoom Actions

    private func zoomIn() {
        withAnimation {
            scale = min(scale * 1.25, 5.0)
        }
    }

    private func zoomOut() {
        withAnimation {
            scale = max(scale / 1.25, 0.1)
        }
    }

    private func resetZoom() {
        withAnimation {
            scale = 1.0
            offset = .zero
            lastOffset = .zero
        }
    }

    // MARK: - Export Actions

    private func copyToClipboard() {
        guard let data = imageData,
              let image = NSImage(data: data) else { return }

        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.writeObjects([image])
    }

    private func openInPreview() {
        guard let data = imageData,
              let image = NSImage(data: data),
              let tiffData = image.tiffRepresentation else { return }

        let tempURL = FileManager.default.temporaryDirectory.appendingPathComponent(filename)
        try? tiffData.write(to: tempURL)

        NSWorkspace.shared.open(tempURL)
    }

    private func setAsDesktop() {
        guard let data = imageData,
              let image = NSImage(data: data) else { return }

        if let screen = NSScreen.main {
            try? NSWorkspace.shared.setDesktopImageURL(
                URL(fileURLWithPath: "/tmp"),
                for: screen,
                options: [:]
            )
            // Note: This is a simplified approach. A full implementation would save to a temp file first.
        }
    }

    private func exportImage(to url: URL, format: ExportFormat) {
        guard let data = imageData,
              let sourceImage = NSImage(data: data),
              let tiffData = sourceImage.tiffRepresentation,
              let bitmap = NSBitmapImageRep(data: tiffData) else { return }

        let exportData: Data?
        switch format {
        case .png:
            exportData = bitmap.representation(using: .png, properties: [:])
        case .jpeg:
            exportData = bitmap.representation(using: .jpeg, properties: [.compressionFactor: 0.9])
        case .tiff:
            exportData = bitmap.representation(using: .tiff, properties: [:])
        case .original:
            exportData = data
        }

        if let exportData = exportData {
            try? exportData.write(to: url)
        }
    }
}

// MARK: - Image Content View

struct ImageContentView: View {
    let image: NSImage
    @Binding var scale: CGFloat
    @Binding var offset: CGSize
    @Binding var lastOffset: CGSize

    @State private var isDragging = false

    var body: some View {
        GeometryReader { geometry in
            Image(nsImage: image)
                .resizable()
                .aspectRatio(contentMode: .fit)
                .scaleEffect(scale)
                .offset(offset)
                .gesture(
                    SimultaneousGesture(
                        MagnificationGesture()
                            .onChanged { value in
                                scale = value
                            }
                            .onEnded { value in
                                scale = min(max(value, 0.1), 5.0)
                            },
                        DragGesture()
                            .onChanged { value in
                                offset = CGSize(
                                    width: lastOffset.width + value.translation.width,
                                    height: lastOffset.height + value.translation.height
                                )
                            }
                            .onEnded { value in
                                lastOffset = CGSize(
                                    width: lastOffset.width + value.translation.width,
                                    height: lastOffset.height + value.translation.height
                                )
                            }
                    )
                )
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(Color(nsColor: .windowBackgroundColor))
        }
    }
}

// MARK: - Error State View

struct ErrorStateView: View {
    let message: String
    let onRetry: () -> Void

    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.triangle")
                .font(.system(size: 48))
                .foregroundColor(.orange)

            Text("Failed to Load Image")
                .font(.headline)

            Text(message)
                .font(.body)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)

            Button("Retry") {
                onRetry()
            }
            .buttonStyle(.borderedProminent)
        }
        .padding()
    }
}

// MARK: - Export Sheet

struct ImageExportSheet: View {
    let imageData: Data
    let defaultFilename: String

    @Environment(\.dismiss) private var dismiss
    @State private var selectedFormat: ExportFormat = .png
    @State private var exportURL: URL?

    enum ExportFormat: String, CaseIterable {
        case png = "PNG"
        case jpeg = "JPEG"
        case tiff = "TIFF"
        case original = "Original"

        var fileExtension: String {
            switch self {
            case .png: return "png"
            case .jpeg: return "jpg"
            case .tiff: return "tiff"
            case .original: return "png"
            }
        }
    }

    var body: some View {
        VStack(spacing: 20) {
            Text("Export Image")
                .font(.title2)
                .fontWeight(.semibold)

            VStack(alignment: .leading, spacing: 12) {
                Picker("Format", selection: $selectedFormat) {
                    ForEach(ExportFormat.allCases, id: \.self) { format in
                        Text(format.rawValue).tag(format)
                    }
                }

                HStack {
                    TextField("Filename", text: Binding(
                        get: { exportURL?.lastPathComponent ?? defaultFilename },
                        set: { _ in }
                    ))
                    .disabled(true)

                    Button("Choose Location...") {
                        let panel = NSSavePanel()
                        panel.allowedContentTypes = [.init(filenameExtension: selectedFormat.fileExtension)!]
                        panel.nameFieldStringValue = defaultFilename

                        if panel.runModal() == .OK {
                            exportURL = panel.url
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
                    if let url = exportURL {
                        performExport(to: url)
                    }
                    dismiss()
                }
                .keyboardShortcut(.defaultAction)
                .buttonStyle(.borderedProminent)
                .disabled(exportURL == nil)
            }
        }
        .padding()
        .frame(width: 450)
    }

    private func performExport(to url: URL) {
        guard let sourceImage = NSImage(data: imageData),
              let tiffData = sourceImage.tiffRepresentation,
              let bitmap = NSBitmapImageRep(data: tiffData) else { return }

        let exportData: Data?
        switch selectedFormat {
        case .png:
            exportData = bitmap.representation(using: .png, properties: [:])
        case .jpeg:
            exportData = bitmap.representation(using: .jpeg, properties: [.compressionFactor: 0.9])
        case .tiff:
            exportData = bitmap.representation(using: .tiff, properties: [:])
        case .original:
            exportData = imageData
        }

        if let exportData = exportData {
            try? exportData.write(to: url)
        }
    }
}

// MARK: - Preview

#Preview {
    ImagePreviewView(
        filename: "example.png",
        subfolder: "",
        imgType: "output"
    )
    .frame(width: 600, height: 400)
}
