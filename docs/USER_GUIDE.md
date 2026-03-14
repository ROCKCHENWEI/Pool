# Pool User Guide

Welcome to Pool, a localized AI video generation engine. This guide will help you get started with creating AI-generated videos on your local machine.

## Table of Contents

- [Installation](#installation)
- [First Launch](#first-launch)
- [Creating Your First Project](#creating-your-first-project)
- [Working with Shots](#working-with-shots)
- [Timeline Editor](#timeline-editor)
- [Node Editor](#node-editor)
- [Model Management](#model-management)
- [API Configuration](#api-configuration)
- [Exporting Videos](#exporting-videos)
- [Troubleshooting](#troubleshooting)

## Installation

### System Requirements

- **macOS**: macOS 12.0 (Monterey) or later
- **Windows**: Windows 10/11 (planned)
- **Memory**: Minimum 16GB RAM (32GB recommended)
- **Storage**: 20GB available space for models
- **GPU**: Metal-compatible GPU (macOS) or CUDA-compatible GPU (planned)

### Installing on macOS

1. **Download the latest release** from the releases page

2. **Mount the DMG file** and drag Pool to your Applications folder

3. **Launch Pool** from Applications or Spotlight

4. **Allow necessary permissions** when prompted:
   - Network access (for API calls)
   - File system access (for project storage)

### Building from Source

If you prefer to build from source:

```bash
# Clone the repository
git clone https://github.com/pool/pool.git
cd pool

# Build the Rust core
cd shared-core
cargo build --release

# Build the macOS app
cd ../apps/macos
swift build -c release
```

## First Launch

When you first launch Pool, you'll be greeted with:

1. **Welcome Screen**: Introduction to Pool's features
2. **Setup Wizard**: Configure your preferences
3. **API Configuration**: Set up your API keys

### Initial Setup

1. Choose a default location for your projects
2. Configure ComfyUI server URL (default: `http://127.0.0.1:8188`)
3. Enter your API keys:
   - **Kling AI**: For video generation
   - **OpenAI**: For text enhancement (optional)

## Creating Your First Project

### Step 1: Create a New Project

1. Click the **"+"** button in the sidebar or choose **File > New Project**
2. Enter a project name (e.g., "My First Video")
3. Add an optional description
4. Click **Create**

### Step 2: Understanding the Interface

The main interface consists of:

- **Sidebar**: Project navigation and quick actions
- **Timeline**: Visual representation of your video shots
- **Properties Panel**: Shot and node properties
- **Toolbar**: Common actions and tools

### Step 3: Add Your First Shot

1. Click **"Add Shot"** in the timeline toolbar
2. Enter a prompt describing your shot:
   ```
   A golden retriever running through a sunny meadow,
   camera following from behind, lens flare
   ```
3. Set the duration (e.g., 5 seconds)
4. Click **"Generate"** to create the shot

## Working with Shots

### Shot Properties

Each shot has the following properties:

| Property | Description |
|----------|-------------|
| **Prompt** | Text description of the shot |
| **Negative Prompt** | Elements to avoid in generation |
| **Duration** | Length of the shot in seconds |
| **Style** | Visual style preset |
| **Quality** | Generation quality setting |

### Managing Shots

- **Reorder**: Drag and drop shots in the timeline
- **Delete**: Select shot and press Delete key
- **Duplicate**: Right-click and select "Duplicate"
- **Edit**: Double-click to edit properties

### Shot Status

Shots progress through these states:

1. **Draft**: Initial state, ready for editing
2. **Pending**: Queued for generation
3. **Processing**: Currently being generated
4. **Completed**: Successfully generated
5. **Failed**: Generation encountered an error

## Timeline Editor

The Timeline Editor provides a visual overview of your project:

### Timeline Controls

- **Play/Pause**: Space bar or click the play button
- **Scrub**: Click and drag on the timeline
- **Zoom**: Pinch gesture or use zoom slider
- **Jump Forward/Back**: Arrow keys (5-second increments)

### Timeline View

- Each colored block represents a shot
- Block width corresponds to shot duration
- Current position indicated by the playhead (red line)

## Node Editor

The Node Editor allows advanced workflow customization:

### Basic Concepts

- **Nodes**: Processing units (input, AI, output)
- **Connections**: Links between nodes
- **Parameters**: Node-specific settings

### Common Node Types

| Node Type | Purpose |
|-----------|---------|
| **Image Input** | Load source images |
| **Video Input** | Load source videos |
| **Style Transfer** | Apply artistic styles |
| **Upscale** | Increase resolution |
| **Video Output** | Export final result |

### Creating Workflows

1. Drag nodes from the library to the canvas
2. Connect outputs to inputs by clicking and dragging
3. Configure node parameters in the properties panel
4. Click "Run Workflow" to execute

## Model Management

Pool supports various AI model types:

### Model Categories

- **Checkpoints**: Base models for generation
- **LoRAs**: Fine-tuned style adapters
- **Embeddings**: Textual inversions
- **VAEs**: Variational autoencoders
- **ControlNet**: Structure guidance models

### Installing Models

1. Open **Model Manager** from the sidebar
2. Browse available models or click **Import**
3. Select the model file (.safetensors recommended)
4. Wait for import to complete

### Model Best Practices

- Keep frequently used models downloaded
- Remove unused models to save space
- Use LoRAs for style variations without full model downloads

## API Configuration

### Kling AI Setup

1. Sign up at [Kling AI](https://kling.ai)
2. Generate an API key from your dashboard
3. Enter the key in **Settings > API Keys > Kling AI**
4. Click "Test Connection" to verify

### OpenAI Setup (Optional)

1. Sign up at [OpenAI](https://openai.com)
2. Generate an API key
3. Enter the key in **Settings > API Keys > OpenAI**
4. Used for:
   - Prompt enhancement
   - Text embeddings
   - Content suggestions

### ComfyUI Setup

1. Install ComfyUI following their [documentation](https://github.com/comfyanonymous/ComfyUI)
2. Start ComfyUI server (default port 8188)
3. Verify connection in **Settings > API Keys > ComfyUI**

## Exporting Videos

### Export Options

| Format | Use Case |
|--------|----------|
| **MP4 (H.264)** | General use, web sharing |
| **MOV (ProRes)** | Professional editing |
| **WebM** | Web embedding |
| **GIF** | Short animations |

### Export Process

1. Ensure all shots are completed
2. Click **Export** in the toolbar
3. Choose format and quality settings
4. Select destination folder
5. Click **Export** and wait for completion

### Quality Settings

- **Low**: 720p, fast encoding
- **Medium**: 1080p, balanced
- **High**: 1440p, high quality
- **Ultra**: 4K, maximum quality

## Troubleshooting

### Common Issues

#### "ComfyUI Connection Failed"

1. Verify ComfyUI is running
2. Check the URL in Settings
3. Ensure no firewall is blocking port 8188

#### "API Key Invalid"

1. Verify the key is correctly entered
2. Check if the key has expired
3. Ensure sufficient API credits

#### "Generation Stuck"

1. Check network connectivity
2. Verify GPU has available memory
3. Try reducing concurrent tasks in Preferences

#### "Out of Memory"

1. Close other GPU-intensive applications
2. Reduce model count
3. Lower generation resolution

### Getting Help

- **Documentation**: [docs/](./)
- **Issues**: [GitHub Issues](https://github.com/pool/pool/issues)
- **Community**: [Discord](https://discord.gg/pool)

### Logs

View logs in **Help > View Logs** for debugging information.

---

**Version**: 0.1.0
**Last Updated**: March 2024
