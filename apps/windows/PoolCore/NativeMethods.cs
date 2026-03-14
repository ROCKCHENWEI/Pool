using System;
using System.Runtime.InteropServices;
using System.Text;

namespace PoolCore;

/// <summary>
/// P/Invoke declarations for Rust FFI interop.
/// These declarations map to the native pool_core library functions.
/// </summary>
public static class NativeMethods
{
    // Use platform-specific library name
#if WINDOWS
    private const string DllName = "pool_core.dll";
#elif LINUX
    private const string DllName = "libpool_core.so";
#else
    private const string DllName = "libpool_core.dylib";
#endif

    #region Version Functions

    /// <summary>
    /// Get the version string of the pool_core library.
    /// </summary>
    /// <returns>Pointer to a null-terminated UTF-8 string.</returns>
    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr pool_version();

    #endregion

    #region Memory Management

    /// <summary>
    /// Free a string allocated by the Rust library.
    /// </summary>
    /// <param name="s">Pointer to the string to free.</param>
    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern void pool_string_free(IntPtr s);

    /// <summary>
    /// Free a byte buffer allocated by the Rust library.
    /// </summary>
    /// <param name="ptr">Pointer to the buffer.</param>
    /// <param name="len">Length of the buffer.</param>
    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern void pool_buffer_free(IntPtr ptr, UIntPtr len);

    #endregion

    #region Project Functions

    /// <summary>
    /// Create a new project.
    /// </summary>
    /// <param name="name">Pointer to UTF-8 encoded project name.</param>
    /// <returns>Pointer to a JSON-encoded project object.</returns>
    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr pool_project_create(IntPtr name);

    /// <summary>
    /// Get a project by ID.
    /// </summary>
    /// <param name="id">Pointer to UTF-8 encoded project ID.</param>
    /// <returns>Pointer to a JSON-encoded project object, or null if not found.</returns>
    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr pool_project_get(IntPtr id);

    /// <summary>
    /// List all projects.
    /// </summary>
    /// <returns>Pointer to a JSON-encoded array of projects.</returns>
    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr pool_project_list();

    /// <summary>
    /// Delete a project by ID.
    /// </summary>
    /// <param name="id">Pointer to UTF-8 encoded project ID.</param>
    /// <returns>1 if successful, 0 if not found.</returns>
    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern int pool_project_delete(IntPtr id);

    #endregion

    #region Shot Functions

    /// <summary>
    /// Create a new shot within a project.
    /// </summary>
    /// <param name="projectId">Pointer to UTF-8 encoded project ID.</param>
    /// <param name="name">Pointer to UTF-8 encoded shot name.</param>
    /// <returns>Pointer to a JSON-encoded shot object.</returns>
    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr pool_shot_create(IntPtr projectId, IntPtr name);

    /// <summary>
    /// Get a shot by ID.
    /// </summary>
    /// <param name="id">Pointer to UTF-8 encoded shot ID.</param>
    /// <returns>Pointer to a JSON-encoded shot object, or null if not found.</returns>
    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr pool_shot_get(IntPtr id);

    /// <summary>
    /// List all shots in a project.
    /// </summary>
    /// <param name="projectId">Pointer to UTF-8 encoded project ID.</param>
    /// <returns>Pointer to a JSON-encoded array of shots.</returns>
    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr pool_shot_list(IntPtr projectId);

    /// <summary>
    /// Update a shot's status.
    /// </summary>
    /// <param name="id">Pointer to UTF-8 encoded shot ID.</param>
    /// <param name="status">Pointer to UTF-8 encoded status string.</param>
    /// <returns>1 if successful, 0 if not found.</returns>
    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern int pool_shot_update_status(IntPtr id, IntPtr status);

    #endregion

    #region Workflow Functions

    /// <summary>
    /// Create a new workflow.
    /// </summary>
    /// <param name="name">Pointer to UTF-8 encoded workflow name.</param>
    /// <param name="definition">Pointer to UTF-8 encoded workflow definition (JSON).</param>
    /// <returns>Pointer to a JSON-encoded workflow object.</returns>
    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr pool_workflow_create(IntPtr name, IntPtr definition);

    /// <summary>
    /// Get a workflow by ID.
    /// </summary>
    /// <param name="id">Pointer to UTF-8 encoded workflow ID.</param>
    /// <returns>Pointer to a JSON-encoded workflow object, or null if not found.</returns>
    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr pool_workflow_get(IntPtr id);

    /// <summary>
    /// List all workflows.
    /// </summary>
    /// <returns>Pointer to a JSON-encoded array of workflows.</returns>
    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr pool_workflow_list();

    /// <summary>
    /// Execute a workflow for a shot.
    /// </summary>
    /// <param name="workflowId">Pointer to UTF-8 encoded workflow ID.</param>
    /// <param name="shotId">Pointer to UTF-8 encoded shot ID.</param>
    /// <param name="params">Pointer to UTF-8 encoded parameters (JSON).</param>
    /// <returns>Pointer to a JSON-encoded execution result.</returns>
    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr pool_workflow_execute(IntPtr workflowId, IntPtr shotId, IntPtr @params);

    #endregion

    #region Helper Methods

    /// <summary>
    /// Convert an IntPtr from Rust to a managed string.
    /// </summary>
    /// <param name="ptr">Pointer to a null-terminated UTF-8 string from Rust.</param>
    /// <returns>Managed string, or null if ptr is zero.</returns>
    public static string? PtrToString(IntPtr ptr)
    {
        if (ptr == IntPtr.Zero)
            return null;

        try
        {
            // Find the null terminator
            int len = 0;
            while (Marshal.ReadByte(ptr, len) != 0)
            {
                len++;
            }

            if (len == 0)
                return string.Empty;

            byte[] buffer = new byte[len];
            Marshal.Copy(ptr, buffer, 0, len);
            return Encoding.UTF8.GetString(buffer);
        }
        finally
        {
            pool_string_free(ptr);
        }
    }

    /// <summary>
    /// Convert a managed string to a Rust-compatible IntPtr.
    /// The caller is responsible for freeing this pointer.
    /// </summary>
    /// <param name="str">The string to convert.</param>
    /// <returns>Pointer to a null-terminated UTF-8 string.</returns>
    public static IntPtr StringToPtr(string str)
    {
        byte[] bytes = Encoding.UTF8.GetBytes(str + "\0");
        IntPtr ptr = Marshal.AllocHGlobal(bytes.Length);
        Marshal.Copy(bytes, 0, ptr, bytes.Length);
        return ptr;
    }

    /// <summary>
    /// Free an IntPtr that was allocated for string passing.
    /// </summary>
    /// <param name="ptr">Pointer to free.</param>
    public static void FreeStringPtr(IntPtr ptr)
    {
        if (ptr != IntPtr.Zero)
        {
            Marshal.FreeHGlobal(ptr);
        }
    }

    #endregion
}
