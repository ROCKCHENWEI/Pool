import SwiftUI

/// Timeline View - Main editing interface for video timeline
/// Displays shots, allows drag-and-drop reordering, and provides playback controls
struct TimelineView: View {
    let project: ProjectItem?

    @State private var selectedShotId: UUID?
    @State private var isPlaying = false
    @State private var currentTime: Double = 0
    @State private var totalDuration: Double = 120 // 2 minutes

    // Sample shots for demonstration
    @State private var shots: [TimelineShot] = [
        TimelineShot(id: UUID(), name: "Opening Scene", duration: 15, thumbnail: "video.fill"),
        TimelineShot(id: UUID(), name: "Product Shot 1", duration: 20, thumbnail: "photo.fill"),
        TimelineShot(id: UUID(), name: "Transition", duration: 5, thumbnail: "arrow.right"),
        TimelineShot(id: UUID(), name: "Demo Sequence", duration: 30, thumbnail: "play.rectangle.fill"),
        TimelineShot(id: UUID(), name: "Closing", duration: 10, thumbnail: "checkmark.circle")
    ]

    var body: some View {
        VStack(spacing: 0) {
            // Header Bar
            headerBar

            Divider()

            // Main Content Area
            HStack(spacing: 0) {
                // Shot List
                shotList
                    .frame(width: 280)
                    .background(Color(nsColor: .controlBackgroundColor))

                Divider()

                // Timeline Canvas
                timelineCanvas
            }

            Divider()

            // Playback Controls
            playbackControls
                .padding()
                .background(Color(nsColor: .windowBackgroundColor))
        }
        .navigationTitle(project?.name ?? "Timeline")
    }

    // MARK: - Header Bar

    private var headerBar: some View {
        HStack {
            // Project Info
            VStack(alignment: .leading, spacing: 2) {
                Text(project?.name ?? "Untitled Project")
                    .font(.headline)
                Text("\(shots.count) shots - \(formatDuration(totalDuration))")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }

            Spacer()

            // Action Buttons
            HStack(spacing: 12) {
                Button(action: { addNewShot() }) {
                    Label("Add Shot", systemImage: "plus")
                }
                .buttonStyle(.bordered)

                Button(action: { }) {
                    Label("Generate All", systemImage: "wand.and.stars")
                }
                .buttonStyle(.borderedProminent)

                Menu {
                    Button("Export Video...", action: { })
                    Button("Export Project...", action: { })
                    Divider()
                    Button("Import Shots...", action: { })
                } label: {
                    Image(systemName: "ellipsis.circle")
                }
                .menuStyle(.borderlessButton)
            }
        }
        .padding(.horizontal)
        .padding(.vertical, 8)
        .background(Color(nsColor: .windowBackgroundColor))
    }

    // MARK: - Shot List

    private var shotList: some View {
        List(selection: $selectedShotId) {
            ForEach(shots) { shot in
                ShotRowView(shot: shot, isSelected: selectedShotId == shot.id)
                    .tag(shot.id)
                    .onTapGesture {
                        selectedShotId = shot.id
                    }
            }
            .onMove { source, destination in
                shots.move(fromOffsets: source, toOffset: destination)
            }
            .onDelete { indexSet in
                shots.remove(atOffsets: indexSet)
            }
        }
        .listStyle(.inset)
    }

    // MARK: - Timeline Canvas

    private var timelineCanvas: some View {
        ZStack {
            // Grid background
            TimelineGridView()

            // Shots visualization
            VStack {
                Spacer()
                ScrollView(.horizontal, showsIndicators: true) {
                    HStack(spacing: 2) {
                        ForEach(Array(shots.enumerated()), id: \.element.id) { index, shot in
                            TimelineBlockView(shot: shot, index: index + 1)
                                .onTapGesture {
                                    selectedShotId = shot.id
                                }
                        }
                    }
                    .padding(.horizontal, 20)
                    .padding(.vertical, 10)
                }
                .frame(height: 100)

                // Playhead
                Rectangle()
                    .fill(Color.red)
                    .frame(width: 2, height: 80)
                    .offset(x: CGFloat(currentTime / totalDuration) * 800)
            }
            .padding()
        }
    }

    // MARK: - Playback Controls

    private var playbackControls: some View {
        HStack(spacing: 20) {
            // Time display
            Text("\(formatDuration(currentTime)) / \(formatDuration(totalDuration))")
                .font(.system(.body, design: .monospaced))
                .frame(width: 120)

            // Playback buttons
            HStack(spacing: 8) {
                Button(action: { currentTime = max(0, currentTime - 5) }) {
                    Image(systemName: "backward.fill")
                }
                .buttonStyle(.borderless)

                Button(action: { isPlaying.toggle() }) {
                    Image(systemName: isPlaying ? "pause.fill" : "play.fill")
                        .font(.title2)
                }
                .buttonStyle(.borderless)

                Button(action: { currentTime = min(totalDuration, currentTime + 5) }) {
                    Image(systemName: "forward.fill")
                }
                .buttonStyle(.borderless)
            }

            // Progress slider
            Slider(value: $currentTime, in: 0...totalDuration)
                .frame(maxWidth: .infinity)

            // Volume
            HStack(spacing: 4) {
                Image(systemName: "speaker.fill")
                Slider(value: .constant(0.8), in: 0...1)
                    .frame(width: 80)
            }
        }
    }

    // MARK: - Helper Methods

    private func addNewShot() {
        let newShot = TimelineShot(
            id: UUID(),
            name: "New Shot \(shots.count + 1)",
            duration: 10,
            thumbnail: "video.fill"
        )
        shots.append(newShot)
    }

    private func formatDuration(_ seconds: Double) -> String {
        let mins = Int(seconds) / 60
        let secs = Int(seconds) % 60
        return String(format: "%d:%02d", mins, secs)
    }
}

// MARK: - Timeline Shot Model

struct TimelineShot: Identifiable {
    let id: UUID
    let name: String
    let duration: Double
    let thumbnail: String
}

// MARK: - Shot Row View

struct ShotRowView: View {
    let shot: TimelineShot
    let isSelected: Bool

    var body: some View {
        HStack(spacing: 12) {
            // Thumbnail placeholder
            RoundedRectangle(cornerRadius: 4)
                .fill(isSelected ? Color.accentColor.opacity(0.3) : Color.gray.opacity(0.2))
                .frame(width: 80, height: 45)
                .overlay(
                    Image(systemName: shot.thumbnail)
                        .foregroundColor(isSelected ? .accentColor : .gray)
                )

            VStack(alignment: .leading, spacing: 4) {
                Text(shot.name)
                    .font(.body)
                    .lineLimit(1)

                Text("\(Int(shot.duration))s")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }

            Spacer()

            // Status indicator
            Circle()
                .fill(Color.green)
                .frame(width: 8, height: 8)
        }
        .padding(4)
        .background(isSelected ? Color.accentColor.opacity(0.1) : Color.clear)
        .cornerRadius(6)
    }
}

// MARK: - Timeline Grid View

struct TimelineGridView: View {
    var body: some View {
        GeometryReader { geometry in
            Path { path in
                // Vertical lines every 50 pixels
                let step: CGFloat = 50
                var x: CGFloat = step
                while x < geometry.size.width {
                    path.move(to: CGPoint(x: x, y: 0))
                    path.addLine(to: CGPoint(x: x, y: geometry.size.height))
                    x += step
                }

                // Horizontal lines every 30 pixels
                var y: CGFloat = 30
                while y < geometry.size.height {
                    path.move(to: CGPoint(x: 0, y: y))
                    path.addLine(to: CGPoint(x: geometry.size.width, y: y))
                    y += 30
                }
            }
            .stroke(Color.gray.opacity(0.1), lineWidth: 1)
        }
    }
}

// MARK: - Timeline Block View

struct TimelineBlockView: View {
    let shot: TimelineShot
    let index: Int
    @State private var isHovered = false

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(shot.name)
                .font(.caption)
                .lineLimit(1)

            HStack(spacing: 4) {
                Text("#\(index)")
                    .font(.caption2)
                    .foregroundColor(.white.opacity(0.7))

                Text("\(Int(shot.duration))s")
                    .font(.caption2)
                    .foregroundColor(.white.opacity(0.7))
            }
        }
        .padding(8)
        .frame(width: CGFloat(shot.duration * 4), height: 60)
        .background(
            RoundedRectangle(cornerRadius: 6)
                .fill(LinearGradient(
                    colors: [Color.accentColor, Color.accentColor.opacity(0.7)],
                    startPoint: .topLeading,
                    endPoint: .bottomTrailing
                ))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 6)
                .stroke(isHovered ? Color.white : Color.clear, lineWidth: 2)
        )
        .onHover { hovering in
            isHovered = hovering
        }
    }
}

// MARK: - Preview

#Preview {
    TimelineView(project: ProjectItem(
        name: "Sample Project",
        shots: 5,
        status: "In Progress",
        lastModified: Date()
    ))
    .frame(width: 1000, height: 600)
}
