import SwiftUI

/// Models View - Model management interface
/// Allows managing AI models, LoRAs, and embeddings
struct ModelsView: View {
    @State private var selectedCategory = "Checkpoints"
    @State private var searchText = ""

    private let categories = [
        "Checkpoints",
        "LoRAs",
        "Embeddings",
        "VAEs",
        "ControlNet"
    ]

    // Sample models for demonstration
    @State private var models: [AIModel] = [
        AIModel(name: "SDXL Base 1.0", type: "checkpoint", size: "6.9 GB", status: .downloaded),
        AIModel(name: "SDXL Refiner", type: "checkpoint", size: "2.6 GB", status: .downloaded),
        AIModel(name: "Realistic Vision V5", type: "checkpoint", size: "2.1 GB", status: .available),
        AIModel(name: "DreamShaper V8", type: "checkpoint", size: "2.1 GB", status: .downloading(progress: 0.45)),
        AIModel(name: "Film Grain LoRA", type: "lora", size: "144 MB", status: .downloaded),
        AIModel(name: "Portrait Helper", type: "lora", size: "72 MB", status: .downloaded)
    ]

    var body: some View {
        NavigationSplitView {
            List(categories, selection: $selectedCategory) { category in
                HStack {
                    Image(systemName: iconForCategory(category))
                        .foregroundColor(.accentColor)
                    Text(category)
                }
                .tag(category)
            }
            .listStyle(.sidebar)
            .navigationTitle("Models")
        } detail: {
            VStack(spacing: 0) {
                // Header
                HStack {
                    Text(selectedCategory)
                        .font(.largeTitle)
                        .fontWeight(.bold)

                    Spacer()

                    Button(action: { }) {
                        Label("Import Model", systemImage: "plus")
                    }
                    .buttonStyle(.bordered)
                }
                .padding()

                Divider()

                // Search bar
                HStack {
                    Image(systemName: "magnifyingglass")
                        .foregroundColor(.secondary)
                    TextField("Search models...", text: $searchText)
                        .textFieldStyle(.plain)
                }
                .padding(8)
                .background(Color(nsColor: .controlBackgroundColor))
                .cornerRadius(6)
                .padding(.horizontal)
                .padding(.vertical, 8)

                // Model list
                List(filteredModels) { model in
                    ModelRowView(model: model)
                }
                .listStyle(.inset)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    private var filteredModels: [AIModel] {
        let typeFilter: String
        switch selectedCategory {
        case "Checkpoints": typeFilter = "checkpoint"
        case "LoRAs": typeFilter = "lora"
        case "Embeddings": typeFilter = "embedding"
        case "VAEs": typeFilter = "vae"
        case "ControlNet": typeFilter = "controlnet"
        default: typeFilter = ""
        }

        return models.filter { model in
            (typeFilter.isEmpty || model.type == typeFilter) &&
            (searchText.isEmpty || model.name.localizedCaseInsensitiveContains(searchText))
        }
    }

    private func iconForCategory(_ category: String) -> String {
        switch category {
        case "Checkpoints": return "cube.box"
        case "LoRAs": return "slider.horizontal.3"
        case "Embeddings": return "vector"
        case "VAEs": return "eye"
        case "ControlNet": return "flowchart"
        default: return "cube"
        }
    }
}

// MARK: - AI Model

struct AIModel: Identifiable {
    let id = UUID()
    let name: String
    let type: String
    let size: String
    let status: ModelStatus

    enum ModelStatus {
        case downloaded
        case available
        case downloading(progress: Double)
    }
}

// MARK: - Model Row View

struct ModelRowView: View {
    let model: AIModel

    var body: some View {
        HStack(spacing: 12) {
            // Icon
            RoundedRectangle(cornerRadius: 8)
                .fill(Color.accentColor.opacity(0.1))
                .frame(width: 48, height: 48)
                .overlay(
                    Image(systemName: "cube.box")
                        .font(.title2)
                        .foregroundColor(.accentColor)
                )

            // Info
            VStack(alignment: .leading, spacing: 4) {
                Text(model.name)
                    .font(.headline)

                HStack(spacing: 8) {
                    Text(model.type.capitalized)
                        .font(.caption)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(Color.secondary.opacity(0.1))
                        .cornerRadius(4)

                    Text(model.size)
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
            }

            Spacer()

            // Status / Actions
            switch model.status {
            case .downloaded:
                HStack(spacing: 8) {
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundColor(.green)
                    Text("Downloaded")
                        .foregroundColor(.secondary)
                }

            case .available:
                Button("Download") {
                    // Start download
                }
                .buttonStyle(.bordered)

            case .downloading(let progress):
                VStack(alignment: .trailing, spacing: 4) {
                    ProgressView(value: progress)
                        .frame(width: 80)
                    Text("\(Int(progress * 100))%")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
            }
        }
        .padding(.vertical, 4)
    }
}

// MARK: - Preview

#Preview {
    ModelsView()
        .frame(width: 800, height: 600)
}
