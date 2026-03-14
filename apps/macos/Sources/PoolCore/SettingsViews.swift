import SwiftUI

/// Settings View - API Keys and service configuration
/// Manages API keys for external services
struct SettingsView: View {
    @State private var apiKeyKling = ""
    @State private var apiKeyOpenAI = ""
    @State private var apiKeyComfyUI = ""
    @State private var comfyUIURL = "http://127.0.0.1:8188"
    @State private var showKeys = false

    var body: some View {
        Form {
            Section {
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        Image(systemName: "video.fill")
                            .foregroundColor(.blue)
                            .frame(width: 24)
                        Text("Kling AI")
                            .fontWeight(.medium)
                        Spacer()
                        Image(systemName: "checkmark.circle.fill")
                            .foregroundColor(.green)
                    }

                    HStack {
                        if showKeys {
                            TextField("API Key", text: $apiKeyKling)
                                .textFieldStyle(.roundedBorder)
                        } else {
                            SecureField("API Key", text: $apiKeyKling)
                                .textFieldStyle(.roundedBorder)
                        }

                        Button(action: { showKeys.toggle() }) {
                            Image(systemName: showKeys ? "eye.slash" : "eye")
                        }
                        .buttonStyle(.borderless)
                    }

                    Text("Required for video generation via Kling AI API")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
                .padding(.vertical, 4)
            } header: {
                Text("Video Generation")
            }

            Section {
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        Image(systemName: "brain")
                            .foregroundColor(.purple)
                            .frame(width: 24)
                        Text("OpenAI")
                            .fontWeight(.medium)
                        Spacer()
                        Image(systemName: "checkmark.circle.fill")
                            .foregroundColor(.green)
                    }

                    SecureField("API Key", text: $apiKeyOpenAI)
                        .textFieldStyle(.roundedBorder)

                    Text("Required for GPT-4 and embedding services")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
                .padding(.vertical, 4)
            } header: {
                Text("LLM Services")
            }

            Section {
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        Image(systemName: "server.rack")
                            .foregroundColor(.orange)
                            .frame(width: 24)
                        Text("ComfyUI Local")
                            .fontWeight(.medium)
                        Spacer()
                        Image(systemName: "checkmark.circle.fill")
                            .foregroundColor(.green)
                    }

                    TextField("Server URL", text: $comfyUIURL)
                        .textFieldStyle(.roundedBorder)

                    Button("Test Connection") {
                        // Test connection
                    }
                    .buttonStyle(.bordered)

                    Text("Local ComfyUI server for image generation")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
                .padding(.vertical, 4)
            } header: {
                Text("Local Services")
            }
        }
        .formStyle(.grouped)
        .navigationTitle("API Keys")
        .frame(width: 500)
    }
}

/// Preferences View - Application preferences
/// General application settings and preferences
struct PreferencesView: View {
    @AppStorage("autoSave") private var autoSave = true
    @AppStorage("autoSaveInterval") private var autoSaveInterval = 5.0
    @AppStorage("showNotifications") private var showNotifications = true
    @AppStorage("darkMode") private var darkMode = true
    @AppStorage("defaultQuality") private var defaultQuality = "High"
    @AppStorage("maxConcurrentTasks") private var maxConcurrentTasks = 4

    private let qualityOptions = ["Low", "Medium", "High", "Ultra"]

    var body: some View {
        Form {
            Section {
                Toggle("Enable Auto-Save", isOn: $autoSave)

                if autoSave {
                    HStack {
                        Text("Auto-Save Interval")
                        Spacer()
                        Text("\(Int(autoSaveInterval)) min")
                            .foregroundColor(.secondary)
                    }
                    Slider(value: $autoSaveInterval, in: 1...30, step: 1)
                }

                Toggle("Show Notifications", isOn: $showNotifications)
            } header: {
                Text("General")
            }

            Section {
                Picker("Default Quality", selection: $defaultQuality) {
                    ForEach(qualityOptions, id: \.self) { option in
                        Text(option)
                    }
                }

                HStack {
                    Text("Max Concurrent Tasks")
                    Spacer()
                    Text("\(maxConcurrentTasks)")
                        .foregroundColor(.secondary)
                }
                Slider(value: Binding(
                    get: { Double(maxConcurrentTasks) },
                    set: { maxConcurrentTasks = Int($0) }
                ), in: 1...8, step: 1)
            } header: {
                Text("Generation")
            }

            Section {
                HStack {
                    Text("Cache Size")
                    Spacer()
                    Text("256 MB")
                        .foregroundColor(.secondary)
                }

                Button("Clear Cache") {
                    // Clear cache
                }
                .buttonStyle(.bordered)

                Button("Reset to Defaults") {
                    // Reset preferences
                }
                .buttonStyle(.bordered)
            } header: {
                Text("Storage")
            }

            Section {
                HStack {
                    Text("Version")
                    Spacer()
                    Text("0.1.0")
                        .foregroundColor(.secondary)
                }

                HStack {
                    Text("Build")
                    Spacer()
                    Text("2024.03.15")
                        .foregroundColor(.secondary)
                }

                Link("Visit GitHub", destination: URL(string: "https://github.com/pool/pool")!)
            } header: {
                Text("About")
            }
        }
        .formStyle(.grouped)
        .navigationTitle("Preferences")
        .frame(width: 500)
    }
}

// MARK: - Preview

#Preview("Settings") {
    SettingsView()
        .frame(width: 500, height: 500)
}

#Preview("Preferences") {
    PreferencesView()
        .frame(width: 500, height: 600)
}
