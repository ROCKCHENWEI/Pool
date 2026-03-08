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
