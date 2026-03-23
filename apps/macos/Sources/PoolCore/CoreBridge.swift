import Foundation

/// CoreBridge provides a Swift interface to the Rust shared-core library.
///
/// This class wraps the C FFI functions and provides an Objective-C compatible
/// Swift interface for use throughout the macOS application.
///
/// Usage:
/// ```swift
/// let version = CoreBridge.version
/// let projectJson = CoreBridge.createProject(name: "My Project")
/// ```
@objc public class CoreBridge: NSObject {

    // MARK: - Version

    /// Get the shared-core library version.
    @objc public static var version: String {
        let cVersion = pool_version()
        defer { pool_string_free(cVersion) }
        return String(cString: cVersion)
    }

    // MARK: - Project Operations

    /// Create a new project with the given name.
    ///
    /// - Parameter name: The project name.
    /// - Returns: A JSON string containing the created project data.
    @objc public static func createProject(name: String) -> String {
        guard let nameC = name.cString(using: .utf8) else {
            return "{\"error\": \"Failed to encode name as UTF-8\"}"
        }

        let result = nameC.withUnsafeBytes { ptr in
            pool_project_create(ptr.baseAddress!.assumingMemoryBound(to: CChar.self))
        }
        defer { pool_string_free(result) }

        return String(cString: result)
    }

    // MARK: - Shot Operations

    /// Create a new shot for a project.
    ///
    /// - Parameters:
    ///   - projectId: The UUID of the parent project.
    ///   - name: The shot name.
    /// - Returns: A JSON string containing the created shot data.
    @objc public static func createShot(projectId: String, name: String) -> String {
        guard let projectIdC = projectId.cString(using: .utf8),
              let nameC = name.cString(using: .utf8) else {
            return "{\"error\": \"Failed to encode parameters as UTF-8\"}"
        }

        let result = projectIdC.withUnsafeBytes { projectIdPtr in
            nameC.withUnsafeBytes { namePtr in
                pool_shot_create(
                    projectIdPtr.baseAddress!.assumingMemoryBound(to: CChar.self),
                    namePtr.baseAddress!.assumingMemoryBound(to: CChar.self)
                )
            }
        }
        defer { pool_string_free(result) }

        return String(cString: result)
    }

    // MARK: - Workflow Operations

    /// Create a new workflow with the given name.
    ///
    /// - Parameter name: The workflow name.
    /// - Returns: A JSON string containing the created workflow data.
    @objc public static func createWorkflow(name: String) -> String {
        guard let nameC = name.cString(using: .utf8) else {
            return "{\"error\": \"Failed to encode name as UTF-8\"}"
        }

        let result = nameC.withUnsafeBytes { ptr in
            pool_workflow_create(ptr.baseAddress!.assumingMemoryBound(to: CChar.self))
        }
        defer { pool_string_free(result) }

        return String(cString: result)
    }

    /// Create a sample workflow with ComfyUI nodes.
    ///
    /// - Returns: A JSON string containing a sample text-to-image workflow.
    @objc public static func createSampleWorkflow() -> String {
        let result = pool_workflow_create_sample()
        defer { pool_string_free(result) }
        return String(cString: result)
    }

    /// Get available node types for workflow creation.
    ///
    /// - Returns: A JSON string containing an array of node type information.
    @objc public static func getWorkflowNodeTypes() -> String {
        let result = pool_workflow_get_node_types()
        defer { pool_string_free(result) }
        return String(cString: result)
    }

    /// Execute a workflow.
    ///
    /// - Parameter workflowJson: A JSON string containing the workflow.
    /// - Returns: A JSON string with execution result.
    @objc public static func executeWorkflow(workflowJson: String) -> String {
        guard let jsonC = workflowJson.cString(using: .utf8) else {
            return "{\"success\":false,\"error\":\"Failed to encode workflow as UTF-8\"}"
        }

        let result = jsonC.withUnsafeBytes { ptr in
            pool_workflow_execute(ptr.baseAddress!.assumingMemoryBound(to: CChar.self))
        }
        defer { pool_string_free(result) }

        return String(cString: result)
    }

    // MARK: - ComfyUI Operations

    /// Set ComfyUI server configuration.
    ///
    /// - Parameters:
    ///   - serverUrl: The ComfyUI server URL.
    ///   - timeoutSecs: Connection timeout in seconds.
    ///   - autoReconnect: Whether to automatically reconnect.
    ///   - maxRetries: Maximum number of retry attempts.
    /// - Returns: A JSON string with success status.
    @objc public static func setComfyUIConfig(
        serverUrl: String,
        timeoutSecs: UInt64,
        autoReconnect: Bool,
        maxRetries: UInt32
    ) -> String {
        guard let urlC = serverUrl.cString(using: .utf8) else {
            return "{\"success\":false,\"error\":\"Failed to encode URL as UTF-8\"}"
        }

        let result = urlC.withUnsafeBytes { ptr in
            pool_comfyui_set_config(
                ptr.baseAddress!.assumingMemoryBound(to: CChar.self),
                timeoutSecs,
                autoReconnect,
                maxRetries
            )
        }
        defer { pool_string_free(result) }

        return String(cString: result)
    }

    /// Get current ComfyUI configuration.
    ///
    /// - Returns: A JSON string containing the configuration.
    @objc public static func getComfyUIConfig() -> String {
        let result = pool_comfyui_get_config()
        defer { pool_string_free(result) }
        return String(cString: result)
    }

    /// Test connection to ComfyUI server.
    ///
    /// - Returns: A JSON string with connection status.
    @objc public static func testComfyUIConnection() -> String {
        let result = pool_comfyui_test_connection()
        defer { pool_string_free(result) }
        return String(cString: result)
    }

    /// Connect to ComfyUI server.
    ///
    /// - Returns: A JSON string with connection status.
    @objc public static func connectComfyUI() -> String {
        let result = pool_comfyui_connect()
        defer { pool_string_free(result) }
        return String(cString: result)
    }

    /// Get available ComfyUI workflow templates.
    ///
    /// - Returns: A JSON string containing an array of templates.
    @objc public static func getComfyUITemplates() -> String {
        let result = pool_comfyui_get_templates()
        defer { pool_string_free(result) }
        return String(cString: result)
    }

    // MARK: - WebSocket Operations

    /// Connect to ComfyUI WebSocket for real-time updates.
    ///
    /// - Returns: A JSON string with connection status.
    @objc public static func websocketConnect() -> String {
        let result = pool_websocket_connect()
        defer { pool_string_free(result) }
        return String(cString: result)
    }

    /// Disconnect from ComfyUI WebSocket.
    @objc public static func websocketDisconnect() {
        pool_websocket_disconnect()
    }

    /// Check if WebSocket is connected.
    ///
    /// - Returns: A JSON string with connection status.
    @objc public static func websocketIsConnected() -> String {
        let result = pool_websocket_is_connected()
        defer { pool_string_free(result) }
        return String(cString: result)
    }

    /// Poll for progress updates (non-blocking).
    ///
    /// - Returns: A JSON string with progress updates array.
    @objc public static func pollProgress() -> String {
        let result = pool_poll_progress()
        defer { pool_string_free(result) }
        return String(cString: result)
    }

    /// Poll for execution updates (non-blocking).
    ///
    /// - Returns: A JSON string with execution updates array.
    @objc public static func pollExecution() -> String {
        let result = pool_poll_execution()
        defer { pool_string_free(result) }
        return String(cString: result)
    }

    // MARK: - Output Files Operations

    /// Get the output directory path.
    ///
    /// - Returns: A JSON string with the path.
    @objc public static func getOutputPath() -> String {
        let result = pool_get_output_path()
        defer { pool_string_free(result) }
        return String(cString: result)
    }

    /// Get list of generated files for a prompt ID.
    ///
    /// - Parameter promptId: The prompt/task ID.
    /// - Returns: A JSON string with files array.
    @objc public static func getGeneratedFiles(promptId: String) -> String {
        guard let promptIdC = promptId.cString(using: .utf8) else {
            return "{\"files\":[]}"
        }

        let result = promptIdC.withUnsafeBytes { ptr in
            pool_get_generated_files(ptr.baseAddress!.assumingMemoryBound(to: CChar.self))
        }
        defer { pool_string_free(result) }
        return String(cString: result)
    }

    // MARK: - Image Data Operations

    /// Get image data from ComfyUI output.
    ///
    /// - Parameters:
    ///   - filename: The image filename.
    ///   - subfolder: The subfolder path (can be empty).
    ///   - imgType: The image type ("output", "input", or "temp").
    /// - Returns: A JSON string with base64-encoded image data.
    @objc public static func getImageData(filename: String, subfolder: String, imgType: String) -> String {
        guard let filenameC = filename.cString(using: .utf8),
              let subfolderC = subfolder.cString(using: .utf8),
              let imgTypeC = imgType.cString(using: .utf8) else {
            return "{\"success\":false,\"error\":\"Failed to encode parameters as UTF-8\"}"
        }

        let result = filenameC.withUnsafeBytes { fPtr in
            subfolderC.withUnsafeBytes { sPtr in
                imgTypeC.withUnsafeBytes { tPtr in
                    pool_get_image_data(
                        fPtr.baseAddress!.assumingMemoryBound(to: CChar.self),
                        sPtr.baseAddress!.assumingMemoryBound(to: CChar.self),
                        tPtr.baseAddress!.assumingMemoryBound(to: CChar.self)
                    )
                }
            }
        }
        defer { pool_string_free(result) }
        return String(cString: result)
    }
}

// MARK: - C Function Declarations

/// Get the library version.
/// Returns a newly allocated C string that must be freed with pool_string_free.
@_silgen_name("pool_version")
func pool_version() -> UnsafeMutablePointer<CChar>

/// Free a string returned by the library.
/// Must be called for all strings returned by pool_* functions.
@_silgen_name("pool_string_free")
func pool_string_free(_ s: UnsafeMutablePointer<CChar>)

/// Create a new project.
/// Returns a newly allocated JSON string that must be freed with pool_string_free.
@_silgen_name("pool_project_create")
func pool_project_create(_ name: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>

/// Create a new shot.
/// Returns a newly allocated JSON string that must be freed with pool_string_free.
@_silgen_name("pool_shot_create")
func pool_shot_create(_ projectId: UnsafePointer<CChar>, _ name: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>

/// Create a new workflow.
/// Returns a newly allocated JSON string that must be freed with pool_string_free.
@_silgen_name("pool_workflow_create")
func pool_workflow_create(_ name: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>

// MARK: - ComfyUI C Functions

/// Set ComfyUI configuration.
/// Returns a JSON string with success status.
@_silgen_name("pool_comfyui_set_config")
func pool_comfyui_set_config(
    _ serverUrl: UnsafePointer<CChar>,
    _ timeoutSecs: UInt64,
    _ autoReconnect: Bool,
    _ maxRetries: UInt32
) -> UnsafeMutablePointer<CChar>

/// Get ComfyUI configuration.
/// Returns a JSON string with configuration.
@_silgen_name("pool_comfyui_get_config")
func pool_comfyui_get_config() -> UnsafeMutablePointer<CChar>

/// Test connection to ComfyUI server.
/// Returns a JSON string with connection status.
@_silgen_name("pool_comfyui_test_connection")
func pool_comfyui_test_connection() -> UnsafeMutablePointer<CChar>

/// Connect to ComfyUI server.
/// Returns a JSON string with connection status.
@_silgen_name("pool_comfyui_connect")
func pool_comfyui_connect() -> UnsafeMutablePointer<CChar>

/// Get ComfyUI workflow templates.
/// Returns a JSON string with templates array.
@_silgen_name("pool_comfyui_get_templates")
func pool_comfyui_get_templates() -> UnsafeMutablePointer<CChar>

// MARK: - Workflow Execution C Functions

/// Execute a workflow.
/// Returns a JSON string with execution result.
@_silgen_name("pool_workflow_execute")
func pool_workflow_execute(_ workflowJson: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>

/// Get available node types.
/// Returns a JSON string with node types array.
@_silgen_name("pool_workflow_get_node_types")
func pool_workflow_get_node_types() -> UnsafeMutablePointer<CChar>

/// Create a sample workflow.
/// Returns a JSON string with sample workflow.
@_silgen_name("pool_workflow_create_sample")
func pool_workflow_create_sample() -> UnsafeMutablePointer<CChar>

// MARK: - WebSocket Progress C Functions

/// Connect to ComfyUI WebSocket for real-time updates.
/// Returns a JSON string with connection status.
@_silgen_name("pool_websocket_connect")
func pool_websocket_connect() -> UnsafeMutablePointer<CChar>

/// Disconnect from ComfyUI WebSocket.
@_silgen_name("pool_websocket_disconnect")
func pool_websocket_disconnect()

/// Check if WebSocket is connected.
/// Returns a JSON string with connection status.
@_silgen_name("pool_websocket_is_connected")
func pool_websocket_is_connected() -> UnsafeMutablePointer<CChar>

/// Poll for progress updates (non-blocking).
/// Returns a JSON string with progress updates array.
@_silgen_name("pool_poll_progress")
func pool_poll_progress() -> UnsafeMutablePointer<CChar>

/// Poll for execution updates (non-blocking).
/// Returns a JSON string with execution updates array.
@_silgen_name("pool_poll_execution")
func pool_poll_execution() -> UnsafeMutablePointer<CChar>

// MARK: - Output Files C Functions

/// Get the output directory path.
/// Returns a JSON string with the path.
@_silgen_name("pool_get_output_path")
func pool_get_output_path() -> UnsafeMutablePointer<CChar>

/// Get list of generated files for a prompt ID.
/// Returns a JSON string with files array.
@_silgen_name("pool_get_generated_files")
func pool_get_generated_files(_ promptId: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>

/// Get image data from ComfyUI output.
/// Returns a JSON string with base64-encoded image data.
@_silgen_name("pool_get_image_data")
func pool_get_image_data(
    _ filename: UnsafePointer<CChar>,
    _ subfolder: UnsafePointer<CChar>,
    _ imgType: UnsafePointer<CChar>
) -> UnsafeMutablePointer<CChar>
