# Pool Windows Client

A Windows desktop client for Pool, built with C# and WPF (Windows Presentation Foundation).

## Features

- **Modern WPF UI**: Dark theme with sidebar navigation
- **Project Management**: Create, view, and manage projects
- **Shot Tracking**: Track individual shots within projects
- **Timeline View**: Visual timeline with playback controls
- **Node Editor**: Visual workflow editor for pipeline tasks
- **Native Rust FFI**: P/Invoke integration with the shared pool_core library

## Requirements

- Windows 10 or later
- .NET 8.0 SDK
- Visual Studio 2022 (recommended) or JetBrains Rider

## Building

### Prerequisites

1. Install .NET 8.0 SDK from https://dotnet.microsoft.com/download
2. (Optional) Install Visual Studio 2022 with .NET desktop development workload

### Build from Command Line

```powershell
cd apps/windows
dotnet restore
dotnet build
```

### Build in Visual Studio

1. Open `Pool.sln` in Visual Studio 2022
2. Right-click the solution in Solution Explorer
3. Select "Restore NuGet Packages"
4. Press F5 to build and run

## Project Structure

```
apps/windows/
├── Pool.sln                    # Visual Studio solution file
├── Pool/                       # WPF application project
│   ├── Pool.csproj            # Project file
│   ├── App.xaml               # Application definition
│   ├── App.xaml.cs            # Application entry point
│   ├── MainWindow.xaml        # Main window layout
│   └── MainWindow.xaml.cs     # Main window code-behind
├── PoolCore/                   # Core library project
│   ├── PoolCore.csproj        # Project file
│   ├── NativeMethods.cs       # P/Invoke FFI declarations
│   ├── Models/                # Data models
│   │   ├── Project.cs         # Project model
│   │   ├── Shot.cs            # Shot model
│   │   └── Workflow.cs        # Workflow model
│   └── Services/              # Business services
│       └── PoolService.cs     # Main service layer
└── README.md                   # This file
```

## Architecture

### Native FFI Integration

The Windows client integrates with the Rust `pool_core` library via P/Invoke:

```csharp
// NativeMethods.cs
[DllImport("pool_core.dll")]
public static extern IntPtr pool_version();

[DllImport("pool_core.dll")]
public static extern IntPtr pool_project_create(IntPtr name);
```

### Service Layer

`PoolService` provides a managed C# API over the native FFI:

```csharp
var service = new PoolService();
var version = service.GetVersion();
var project = service.CreateProject("My Project");
```

### MVVM Pattern

The application follows MVVM (Model-View-ViewModel) principles:

- **Model**: `Project`, `Shot`, `Workflow` classes in PoolCore
- **View**: XAML files with data binding
- **ViewModel**: Code-behind with `INotifyPropertyChanged`

## UI Components

### Main Window Layout

```
+-----------+-------------------+------------+
|           |                   |            |
|  Sidebar  |    Content Area   |  Properties|
|  (240px)  |                   |   (300px)  |
|           |                   |            |
|  Projects |  Projects View    |  Project   |
|  Shots    |  Timeline View    |  Info      |
|  Timeline |  Node Editor View |            |
|  Nodes    |                   |            |
|           +-------------------+            |
|           |    Status Bar     |            |
+-----------+-------------------+------------+
```

### Views

1. **Projects View**: Grid of project cards with thumbnails
2. **Timeline View**: Multi-track timeline with playback controls
3. **Node Editor**: Visual node graph with connection lines

## Theming

The application uses a dark theme defined in `App.xaml`:

- Background: `#1E1E1E`
- Surface: `#2D2D2D`
- Card: `#3C3C3C`
- Primary: `#0078D4` (Windows Blue)
- Text Primary: `#FFFFFF`
- Text Secondary: `#B4B4B4`

## Dependencies

- `Microsoft.Extensions.DependencyInjection` (8.0.0): Dependency injection

## Configuration

The application uses `appsettings.json` for configuration (to be added):

```json
{
  "PoolCore": {
    "LibraryPath": "pool_core.dll"
  },
  "Database": {
    "Path": "%LOCALAPPDATA%\\Pool\\pool.db"
  }
}
```

## Deployment

### Build Release

```powershell
dotnet build -c Release
dotnet publish -c Release -r win-x64 --self-contained
```

### Create MSI Installer

Use the Visual Studio Setup Project or WiX Toolset to create an MSI installer.

## Integration with Pool Core

To use the native Rust library:

1. Build the Rust library for Windows:
   ```bash
   cd shared-core
   cargo build --release --target x86_64-pc-windows-msvc
   ```

2. Copy `pool_core.dll` to the application directory

3. The application will automatically load the native library via P/Invoke

## Troubleshooting

### DllNotFoundException

If you see `DllNotFoundException: Unable to load DLL 'pool_core.dll'`:

1. Ensure `pool_core.dll` is in the same directory as `Pool.exe`
2. Check that the DLL was built for x64 (or match your build configuration)
3. Install the Visual C++ Redistributable if needed

### Build Errors

If you get build errors:

1. Ensure .NET 8.0 SDK is installed: `dotnet --version`
2. Clear NuGet cache: `dotnet nuget locals all --clear`
3. Delete `bin` and `obj` folders and rebuild

## Future Enhancements

- [ ] Full MVVM with view models
- [ ] Async/await for all FFI calls
- [ ] Auto-update functionality
- [ ] System tray icon
- [ ] File associations for .pool files
- [ ] Plugin architecture for custom nodes

## License

MIT License - See root LICENSE file for details.
