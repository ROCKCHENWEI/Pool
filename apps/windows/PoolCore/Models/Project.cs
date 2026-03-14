using System;

namespace PoolCore.Models;

/// <summary>
/// Represents a project in the Pool system.
/// </summary>
public class Project
{
    /// <summary>
    /// Unique identifier for the project.
    /// </summary>
    public string Id { get; set; } = string.Empty;

    /// <summary>
    /// Display name of the project.
    /// </summary>
    public string Name { get; set; } = string.Empty;

    /// <summary>
    /// Description of the project.
    /// </summary>
    public string Description { get; set; } = string.Empty;

    /// <summary>
    /// Number of shots in this project.
    /// </summary>
    public int ShotCount { get; set; }

    /// <summary>
    /// Current status of the project.
    /// </summary>
    public ProjectStatus Status { get; set; } = ProjectStatus.Active;

    /// <summary>
    /// Creation timestamp.
    /// </summary>
    public DateTime CreatedAt { get; set; } = DateTime.UtcNow;

    /// <summary>
    /// Last update timestamp.
    /// </summary>
    public DateTime UpdatedAt { get; set; } = DateTime.UtcNow;

    /// <summary>
    /// Path to the project folder on disk.
    /// </summary>
    public string Path { get; set; } = string.Empty;

    /// <summary>
    /// Associated workflow template ID.
    /// </summary>
    public string? WorkflowId { get; set; }

    /// <summary>
    /// Project metadata as key-value pairs.
    /// </summary>
    public Dictionary<string, string> Metadata { get; set; } = new();
}

/// <summary>
/// Project status enumeration.
/// </summary>
public enum ProjectStatus
{
    /// <summary>
    /// Project is active and being worked on.
    /// </summary>
    Active,

    /// <summary>
    /// Project is on hold.
    /// </summary>
    OnHold,

    /// <summary>
    /// Project has been completed.
    /// </summary>
    Completed,

    /// <summary>
    /// Project has been archived.
    /// </summary>
    Archived
}
