using System;

namespace PoolCore.Models;

/// <summary>
/// Represents a workflow template in the Pool system.
/// </summary>
public class Workflow
{
    /// <summary>
    /// Unique identifier for the workflow.
    /// </summary>
    public string Id { get; set; } = string.Empty;

    /// <summary>
    /// Display name of the workflow.
    /// </summary>
    public string Name { get; set; } = string.Empty;

    /// <summary>
    /// Description of the workflow.
    /// </summary>
    public string Description { get; set; } = string.Empty;

    /// <summary>
    /// Workflow category.
    /// </summary>
    public string Category { get; set; } = string.Empty;

    /// <summary>
    /// JSON definition of the workflow nodes and connections.
    /// </summary>
    public string Definition { get; set; } = "{}";

    /// <summary>
    /// Creation timestamp.
    /// </summary>
    public DateTime CreatedAt { get; set; } = DateTime.UtcNow;

    /// <summary>
    /// Last update timestamp.
    /// </summary>
    public DateTime UpdatedAt { get; set; } = DateTime.UtcNow;

    /// <summary>
    /// Whether this is a built-in workflow template.
    /// </summary>
    public bool IsBuiltIn { get; set; }

    /// <summary>
    /// List of nodes in this workflow.
    /// </summary>
    public List<WorkflowNode> Nodes { get; set; } = new();
}

/// <summary>
/// Represents a node in a workflow.
/// </summary>
public class WorkflowNode
{
    /// <summary>
    /// Unique identifier for the node within the workflow.
    /// </summary>
    public string Id { get; set; } = string.Empty;

    /// <summary>
    /// Type of the node (e.g., "input", "process", "output").
    /// </summary>
    public string Type { get; set; } = string.Empty;

    /// <summary>
    /// Display name of the node.
    /// </summary>
    public string Name { get; set; } = string.Empty;

    /// <summary>
    /// X position in the node editor.
    /// </summary>
    public double X { get; set; }

    /// <summary>
    /// Y position in the node editor.
    /// </summary>
    public double Y { get; set; }

    /// <summary>
    /// Node configuration as JSON.
    /// </summary>
    public string Config { get; set; } = "{}";

    /// <summary>
    /// Input ports.
    /// </summary>
    public List<WorkflowPort> Inputs { get; set; } = new();

    /// <summary>
    /// Output ports.
    /// </summary>
    public List<WorkflowPort> Outputs { get; set; } = new();
}

/// <summary>
/// Represents a port on a workflow node.
/// </summary>
public class WorkflowPort
{
    /// <summary>
    /// Port name.
    /// </summary>
    public string Name { get; set; } = string.Empty;

    /// <summary>
    /// Data type of the port.
    /// </summary>
    public string DataType { get; set; } = "any";

    /// <summary>
    /// Whether this port is required.
    /// </summary>
    public bool Required { get; set; }
}

/// <summary>
/// Represents a connection between workflow nodes.
/// </summary>
public class WorkflowConnection
{
    /// <summary>
    /// Source node ID.
    /// </summary>
    public string SourceNodeId { get; set; } = string.Empty;

    /// <summary>
    /// Source port name.
    /// </summary>
    public string SourcePort { get; set; } = string.Empty;

    /// <summary>
    /// Target node ID.
    /// </summary>
    public string TargetNodeId { get; set; } = string.Empty;

    /// <summary>
    /// Target port name.
    /// </summary>
    public string TargetPort { get; set; } = string.Empty;
}

/// <summary>
/// Represents a workflow execution instance.
/// </summary>
public class WorkflowExecution
{
    /// <summary>
    /// Unique identifier for this execution.
    /// </summary>
    public string Id { get; set; } = string.Empty;

    /// <summary>
    /// ID of the workflow being executed.
    /// </summary>
    public string WorkflowId { get; set; } = string.Empty;

    /// <summary>
    /// ID of the shot being processed.
    /// </summary>
    public string ShotId { get; set; } = string.Empty;

    /// <summary>
    /// Current status of the execution.
    /// </summary>
    public ExecutionStatus Status { get; set; } = ExecutionStatus.Pending;

    /// <summary>
    /// Execution start time.
    /// </summary>
    public DateTime? StartedAt { get; set; }

    /// <summary>
    /// Execution end time.
    /// </summary>
    public DateTime? CompletedAt { get; set; }

    /// <summary>
    /// Error message if execution failed.
    /// </summary>
    public string? ErrorMessage { get; set; }

    /// <summary>
    /// Execution progress (0-100).
    /// </summary>
    public int Progress { get; set; }

    /// <summary>
    /// Execution result as JSON.
    /// </summary>
    public string? Result { get; set; }
}

/// <summary>
/// Workflow execution status enumeration.
/// </summary>
public enum ExecutionStatus
{
    /// <summary>
    /// Execution is pending.
    /// </summary>
    Pending,

    /// <summary>
    /// Execution is running.
    /// </summary>
    Running,

    /// <summary>
    /// Execution completed successfully.
    /// </summary>
    Completed,

    /// <summary>
    /// Execution failed.
    /// </summary>
    Failed,

    /// <summary>
    /// Execution was cancelled.
    /// </summary>
    Cancelled
}
