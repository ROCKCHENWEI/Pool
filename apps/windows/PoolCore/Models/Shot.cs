using System;

namespace PoolCore.Models;

/// <summary>
/// Represents a shot within a project.
/// </summary>
public class Shot
{
    /// <summary>
    /// Unique identifier for the shot.
    /// </summary>
    public string Id { get; set; } = string.Empty;

    /// <summary>
    /// ID of the parent project.
    /// </summary>
    public string ProjectId { get; set; } = string.Empty;

    /// <summary>
    /// Display name of the shot (e.g., "SH001").
    /// </summary>
    public string Name { get; set; } = string.Empty;

    /// <summary>
    /// Description of the shot.
    /// </summary>
    public string Description { get; set; } = string.Empty;

    /// <summary>
    /// Current status of the shot.
    /// </summary>
    public ShotStatus Status { get; set; } = ShotStatus.NotStarted;

    /// <summary>
    /// Start frame number.
    /// </summary>
    public int StartFrame { get; set; }

    /// <summary>
    /// End frame number.
    /// </summary>
    public int EndFrame { get; set; }

    /// <summary>
    /// Frame rate for this shot.
    /// </summary>
    public int FrameRate { get; set; } = 24;

    /// <summary>
    /// Duration in frames.
    /// </summary>
    public int DurationFrames => EndFrame - StartFrame + 1;

    /// <summary>
    /// Duration in seconds.
    /// </summary>
    public double DurationSeconds => DurationFrames / (double)FrameRate;

    /// <summary>
    /// Resolution width in pixels.
    /// </summary>
    public int Width { get; set; } = 1920;

    /// <summary>
    /// Resolution height in pixels.
    /// </summary>
    public int Height { get; set; } = 1080;

    /// <summary>
    /// Path to the shot folder on disk.
    /// </summary>
    public string Path { get; set; } = string.Empty;

    /// <summary>
    /// Creation timestamp.
    /// </summary>
    public DateTime CreatedAt { get; set; } = DateTime.UtcNow;

    /// <summary>
    /// Last update timestamp.
    /// </summary>
    public DateTime UpdatedAt { get; set; } = DateTime.UtcNow;

    /// <summary>
    /// Assigned artist.
    /// </summary>
    public string? AssignedTo { get; set; }

    /// <summary>
    /// Shot metadata as key-value pairs.
    /// </summary>
    public Dictionary<string, string> Metadata { get; set; } = new();

    /// <summary>
    /// List of file versions for this shot.
    /// </summary>
    public List<ShotVersion> Versions { get; set; } = new();
}

/// <summary>
/// Shot status enumeration.
/// </summary>
public enum ShotStatus
{
    /// <summary>
    /// Shot has not been started.
    /// </summary>
    NotStarted,

    /// <summary>
    /// Shot is in progress.
    /// </summary>
    InProgress,

    /// <summary>
    /// Shot is pending review.
    /// </summary>
    PendingReview,

    /// <summary>
    /// Shot needs revisions.
    /// </summary>
    NeedsRevision,

    /// <summary>
    /// Shot has been approved.
    /// </summary>
    Approved,

    /// <summary>
    /// Shot is final.
    /// </summary>
    Final
}

/// <summary>
/// Represents a version of a shot.
/// </summary>
public class ShotVersion
{
    /// <summary>
    /// Version number.
    /// </summary>
    public int Version { get; set; }

    /// <summary>
    /// Path to the version file.
    /// </summary>
    public string Path { get; set; } = string.Empty;

    /// <summary>
    /// Creation timestamp.
    /// </summary>
    public DateTime CreatedAt { get; set; } = DateTime.UtcNow;

    /// <summary>
    /// User who created this version.
    /// </summary>
    public string? CreatedBy { get; set; }

    /// <summary>
    /// Comment for this version.
    /// </summary>
    public string? Comment { get; set; }
}
