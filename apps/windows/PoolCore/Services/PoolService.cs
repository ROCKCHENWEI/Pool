using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text.Json;
using PoolCore.Models;

namespace PoolCore.Services;

/// <summary>
/// Service layer for Pool operations.
/// Provides a managed C# API over the native Rust FFI.
/// </summary>
public class PoolService : IDisposable
{
    private bool _disposed;
    private readonly JsonSerializerOptions _jsonOptions;

    public PoolService()
    {
        _jsonOptions = new JsonSerializerOptions
        {
            PropertyNameCaseInsensitive = true,
            PropertyNamingPolicy = JsonNamingPolicy.CamelCase
        };
    }

    #region Version

    /// <summary>
    /// Get the version of the pool_core library.
    /// </summary>
    /// <returns>Version string.</returns>
    public string GetVersion()
    {
        try
        {
            var ptr = NativeMethods.pool_version();
            return NativeMethods.PtrToString(ptr) ?? "unknown";
        }
        catch (DllNotFoundException)
        {
            // Fallback if native library is not available
            return "1.0.0 (managed)";
        }
        catch (EntryPointNotFoundException)
        {
            return "1.0.0 (managed)";
        }
    }

    #endregion

    #region Projects

    /// <summary>
    /// Create a new project.
    /// </summary>
    /// <param name="name">Project name.</param>
    /// <returns>The created project.</returns>
    public Project CreateProject(string name)
    {
        try
        {
            var namePtr = NativeMethods.StringToPtr(name);
            try
            {
                var resultPtr = NativeMethods.pool_project_create(namePtr);
                var json = NativeMethods.PtrToString(resultPtr);
                return JsonSerializer.Deserialize<Project>(json ?? "{}", _jsonOptions)
                    ?? new Project { Name = name };
            }
            finally
            {
                NativeMethods.FreeStringPtr(namePtr);
            }
        }
        catch (DllNotFoundException)
        {
            // Fallback: create a managed-only project
            return new Project
            {
                Id = Guid.NewGuid().ToString(),
                Name = name,
                CreatedAt = DateTime.UtcNow,
                UpdatedAt = DateTime.UtcNow
            };
        }
        catch (EntryPointNotFoundException)
        {
            return new Project
            {
                Id = Guid.NewGuid().ToString(),
                Name = name,
                CreatedAt = DateTime.UtcNow,
                UpdatedAt = DateTime.UtcNow
            };
        }
    }

    /// <summary>
    /// Get a project by ID.
    /// </summary>
    /// <param name="id">Project ID.</param>
    /// <returns>The project, or null if not found.</returns>
    public Project? GetProject(string id)
    {
        try
        {
            var idPtr = NativeMethods.StringToPtr(id);
            try
            {
                var resultPtr = NativeMethods.pool_project_get(idPtr);
                var json = NativeMethods.PtrToString(resultPtr);
                return json != null
                    ? JsonSerializer.Deserialize<Project>(json, _jsonOptions)
                    : null;
            }
            finally
            {
                NativeMethods.FreeStringPtr(idPtr);
            }
        }
        catch (DllNotFoundException)
        {
            return null;
        }
        catch (EntryPointNotFoundException)
        {
            return null;
        }
    }

    /// <summary>
    /// List all projects.
    /// </summary>
    /// <returns>List of all projects.</returns>
    public List<Project> ListProjects()
    {
        try
        {
            var resultPtr = NativeMethods.pool_project_list();
            var json = NativeMethods.PtrToString(resultPtr);
            return json != null
                ? JsonSerializer.Deserialize<List<Project>>(json, _jsonOptions) ?? new List<Project>()
                : new List<Project>();
        }
        catch (DllNotFoundException)
        {
            return new List<Project>();
        }
        catch (EntryPointNotFoundException)
        {
            return new List<Project>();
        }
    }

    /// <summary>
    /// Delete a project by ID.
    /// </summary>
    /// <param name="id">Project ID.</param>
    /// <returns>True if deleted, false if not found.</returns>
    public bool DeleteProject(string id)
    {
        try
        {
            var idPtr = NativeMethods.StringToPtr(id);
            try
            {
                return NativeMethods.pool_project_delete(idPtr) != 0;
            }
            finally
            {
                NativeMethods.FreeStringPtr(idPtr);
            }
        }
        catch (DllNotFoundException)
        {
            return false;
        }
        catch (EntryPointNotFoundException)
        {
            return false;
        }
    }

    #endregion

    #region Shots

    /// <summary>
    /// Create a new shot within a project.
    /// </summary>
    /// <param name="projectId">Parent project ID.</param>
    /// <param name="name">Shot name.</param>
    /// <returns>The created shot.</returns>
    public Shot CreateShot(string projectId, string name)
    {
        try
        {
            var projectIdPtr = NativeMethods.StringToPtr(projectId);
            var namePtr = NativeMethods.StringToPtr(name);
            try
            {
                var resultPtr = NativeMethods.pool_shot_create(projectIdPtr, namePtr);
                var json = NativeMethods.PtrToString(resultPtr);
                return JsonSerializer.Deserialize<Shot>(json ?? "{}", _jsonOptions)
                    ?? new Shot { ProjectId = projectId, Name = name };
            }
            finally
            {
                NativeMethods.FreeStringPtr(projectIdPtr);
                NativeMethods.FreeStringPtr(namePtr);
            }
        }
        catch (DllNotFoundException)
        {
            return new Shot
            {
                Id = Guid.NewGuid().ToString(),
                ProjectId = projectId,
                Name = name,
                CreatedAt = DateTime.UtcNow,
                UpdatedAt = DateTime.UtcNow
            };
        }
        catch (EntryPointNotFoundException)
        {
            return new Shot
            {
                Id = Guid.NewGuid().ToString(),
                ProjectId = projectId,
                Name = name,
                CreatedAt = DateTime.UtcNow,
                UpdatedAt = DateTime.UtcNow
            };
        }
    }

    /// <summary>
    /// Get a shot by ID.
    /// </summary>
    /// <param name="id">Shot ID.</param>
    /// <returns>The shot, or null if not found.</returns>
    public Shot? GetShot(string id)
    {
        try
        {
            var idPtr = NativeMethods.StringToPtr(id);
            try
            {
                var resultPtr = NativeMethods.pool_shot_get(idPtr);
                var json = NativeMethods.PtrToString(resultPtr);
                return json != null
                    ? JsonSerializer.Deserialize<Shot>(json, _jsonOptions)
                    : null;
            }
            finally
            {
                NativeMethods.FreeStringPtr(idPtr);
            }
        }
        catch (DllNotFoundException)
        {
            return null;
        }
        catch (EntryPointNotFoundException)
        {
            return null;
        }
    }

    /// <summary>
    /// List all shots in a project.
    /// </summary>
    /// <param name="projectId">Project ID.</param>
    /// <returns>List of shots in the project.</returns>
    public List<Shot> ListShots(string projectId)
    {
        try
        {
            var projectIdPtr = NativeMethods.StringToPtr(projectId);
            try
            {
                var resultPtr = NativeMethods.pool_shot_list(projectIdPtr);
                var json = NativeMethods.PtrToString(resultPtr);
                return json != null
                    ? JsonSerializer.Deserialize<List<Shot>>(json, _jsonOptions) ?? new List<Shot>()
                    : new List<Shot>();
            }
            finally
            {
                NativeMethods.FreeStringPtr(projectIdPtr);
            }
        }
        catch (DllNotFoundException)
        {
            return new List<Shot>();
        }
        catch (EntryPointNotFoundException)
        {
            return new List<Shot>();
        }
    }

    /// <summary>
    /// Update a shot's status.
    /// </summary>
    /// <param name="id">Shot ID.</param>
    /// <param name="status">New status.</param>
    /// <returns>True if updated, false if not found.</returns>
    public bool UpdateShotStatus(string id, ShotStatus status)
    {
        try
        {
            var idPtr = NativeMethods.StringToPtr(id);
            var statusPtr = NativeMethods.StringToPtr(status.ToString());
            try
            {
                return NativeMethods.pool_shot_update_status(idPtr, statusPtr) != 0;
            }
            finally
            {
                NativeMethods.FreeStringPtr(idPtr);
                NativeMethods.FreeStringPtr(statusPtr);
            }
        }
        catch (DllNotFoundException)
        {
            return false;
        }
        catch (EntryPointNotFoundException)
        {
            return false;
        }
    }

    #endregion

    #region Workflows

    /// <summary>
    /// Create a new workflow.
    /// </summary>
    /// <param name="name">Workflow name.</param>
    /// <param name="definition">Workflow definition (JSON).</param>
    /// <returns>The created workflow.</returns>
    public Workflow CreateWorkflow(string name, string definition)
    {
        try
        {
            var namePtr = NativeMethods.StringToPtr(name);
            var definitionPtr = NativeMethods.StringToPtr(definition);
            try
            {
                var resultPtr = NativeMethods.pool_workflow_create(namePtr, definitionPtr);
                var json = NativeMethods.PtrToString(resultPtr);
                return JsonSerializer.Deserialize<Workflow>(json ?? "{}", _jsonOptions)
                    ?? new Workflow { Name = name, Definition = definition };
            }
            finally
            {
                NativeMethods.FreeStringPtr(namePtr);
                NativeMethods.FreeStringPtr(definitionPtr);
            }
        }
        catch (DllNotFoundException)
        {
            return new Workflow
            {
                Id = Guid.NewGuid().ToString(),
                Name = name,
                Definition = definition,
                CreatedAt = DateTime.UtcNow,
                UpdatedAt = DateTime.UtcNow
            };
        }
        catch (EntryPointNotFoundException)
        {
            return new Workflow
            {
                Id = Guid.NewGuid().ToString(),
                Name = name,
                Definition = definition,
                CreatedAt = DateTime.UtcNow,
                UpdatedAt = DateTime.UtcNow
            };
        }
    }

    /// <summary>
    /// Get a workflow by ID.
    /// </summary>
    /// <param name="id">Workflow ID.</param>
    /// <returns>The workflow, or null if not found.</returns>
    public Workflow? GetWorkflow(string id)
    {
        try
        {
            var idPtr = NativeMethods.StringToPtr(id);
            try
            {
                var resultPtr = NativeMethods.pool_workflow_get(idPtr);
                var json = NativeMethods.PtrToString(resultPtr);
                return json != null
                    ? JsonSerializer.Deserialize<Workflow>(json, _jsonOptions)
                    : null;
            }
            finally
            {
                NativeMethods.FreeStringPtr(idPtr);
            }
        }
        catch (DllNotFoundException)
        {
            return null;
        }
        catch (EntryPointNotFoundException)
        {
            return null;
        }
    }

    /// <summary>
    /// List all workflows.
    /// </summary>
    /// <returns>List of all workflows.</returns>
    public List<Workflow> ListWorkflows()
    {
        try
        {
            var resultPtr = NativeMethods.pool_workflow_list();
            var json = NativeMethods.PtrToString(resultPtr);
            return json != null
                ? JsonSerializer.Deserialize<List<Workflow>>(json, _jsonOptions) ?? new List<Workflow>()
                : new List<Workflow>();
        }
        catch (DllNotFoundException)
        {
            return new List<Workflow>();
        }
        catch (EntryPointNotFoundException)
        {
            return new List<Workflow>();
        }
    }

    /// <summary>
    /// Execute a workflow for a shot.
    /// </summary>
    /// <param name="workflowId">Workflow ID.</param>
    /// <param name="shotId">Shot ID.</param>
    /// <param name="parameters">Execution parameters (JSON).</param>
    /// <returns>Execution result.</returns>
    public WorkflowExecution ExecuteWorkflow(string workflowId, string shotId, string parameters = "{}")
    {
        try
        {
            var workflowIdPtr = NativeMethods.StringToPtr(workflowId);
            var shotIdPtr = NativeMethods.StringToPtr(shotId);
            var parametersPtr = NativeMethods.StringToPtr(parameters);
            try
            {
                var resultPtr = NativeMethods.pool_workflow_execute(workflowIdPtr, shotIdPtr, parametersPtr);
                var json = NativeMethods.PtrToString(resultPtr);
                return JsonSerializer.Deserialize<WorkflowExecution>(json ?? "{}", _jsonOptions)
                    ?? new WorkflowExecution { WorkflowId = workflowId, ShotId = shotId };
            }
            finally
            {
                NativeMethods.FreeStringPtr(workflowIdPtr);
                NativeMethods.FreeStringPtr(shotIdPtr);
                NativeMethods.FreeStringPtr(parametersPtr);
            }
        }
        catch (DllNotFoundException)
        {
            return new WorkflowExecution
            {
                Id = Guid.NewGuid().ToString(),
                WorkflowId = workflowId,
                ShotId = shotId,
                Status = ExecutionStatus.Failed,
                ErrorMessage = "Native library not available"
            };
        }
        catch (EntryPointNotFoundException)
        {
            return new WorkflowExecution
            {
                Id = Guid.NewGuid().ToString(),
                WorkflowId = workflowId,
                ShotId = shotId,
                Status = ExecutionStatus.Failed,
                ErrorMessage = "Native function not found"
            };
        }
    }

    #endregion

    #region IDisposable

    public void Dispose()
    {
        Dispose(true);
        GC.SuppressFinalize(this);
    }

    protected virtual void Dispose(bool disposing)
    {
        if (!_disposed)
        {
            if (disposing)
            {
                // Dispose managed resources
            }

            // Dispose unmanaged resources
            _disposed = true;
        }
    }

    ~PoolService()
    {
        Dispose(false);
    }

    #endregion
}
