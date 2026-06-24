use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSnapshot {
    pub version: u32,
    pub generated_at: String,
    pub project_filter: Option<String>,
    pub stats: RuntimeSnapshotStats,
    pub projects: Vec<ProjectSnapshot>,
    pub workflows: Vec<WorkflowSnapshot>,
    pub node_states: Vec<NodeRuntimeState>,
    pub tasks: Vec<TaskSnapshot>,
    pub assets: Vec<AssetSnapshot>,
    pub events: Vec<EventSnapshot>,
    pub provider_requests: Vec<ProviderRequestSnapshot>,
    pub software_actions: Vec<SoftwareActionSnapshot>,
    pub agent_sessions: Vec<AgentSessionSnapshot>,
    pub api_keys: Vec<ApiKeySnapshot>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeSnapshotStats {
    pub projects: usize,
    pub workflows: usize,
    pub tasks: usize,
    pub assets: usize,
    pub events: usize,
    pub provider_requests: usize,
    pub software_actions: usize,
    pub agent_sessions: usize,
    pub api_keys: usize,
    pub waiting_approval: usize,
    pub running: usize,
    pub failed: usize,
    pub task_estimated_tokens: u64,
    pub waiting_approval_estimated_tokens: u64,
    pub agent_token_used: u64,
    pub agent_token_budget: u64,
    pub token_total: u64,
    pub budget_remaining: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSnapshot {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub status: Option<String>,
    pub settings: Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowSnapshot {
    pub id: String,
    pub project_id: Option<String>,
    pub shot_id: Option<String>,
    pub name: Option<String>,
    pub nodes: Value,
    pub connections: Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeRuntimeState {
    pub node_id: String,
    pub task_id: String,
    pub title: String,
    pub status: String,
    pub provider_id: Option<String>,
    pub requires_approval: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSnapshot {
    pub id: String,
    pub project_slug: String,
    pub node_id: Option<String>,
    pub title: String,
    pub status: String,
    pub provider_id: Option<String>,
    pub cost_estimate_tokens: u64,
    pub requires_approval: bool,
    pub request_metadata_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetSnapshot {
    pub id: String,
    pub project_slug: String,
    pub name: String,
    pub asset_type: String,
    pub local_path: String,
    pub source_node_id: Option<String>,
    pub provider_url: Option<String>,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSnapshot {
    pub id: String,
    pub project_slug: String,
    pub level: String,
    pub message: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRequestSnapshot {
    pub id: String,
    pub task_id: String,
    pub project_slug: Option<String>,
    pub provider_id: String,
    pub request: Value,
    pub response: Option<Value>,
    pub metadata_path: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareActionSnapshot {
    pub id: String,
    pub task_id: Option<String>,
    pub adapter_id: String,
    pub action_kind: String,
    pub command: Value,
    pub verification: Option<Value>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSessionSnapshot {
    pub id: String,
    pub project_slug: String,
    pub tools: Value,
    pub token_budget: Option<u64>,
    pub token_used: u64,
    pub transcript_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeySnapshot {
    pub id: String,
    pub provider: String,
    pub service_type: String,
    pub configured: bool,
    pub key_hint: Option<String>,
    pub metadata: Value,
    pub created_at: String,
    pub updated_at: String,
}

pub fn build_runtime_snapshot(
    connection: &Connection,
    project_slug: Option<&str>,
) -> Result<RuntimeSnapshot> {
    let projects = query_projects(connection, project_slug)?;
    let workflows = query_workflows(connection, project_slug)?;
    let tasks = query_tasks(connection, project_slug)?;
    let assets = query_assets(connection, project_slug)?;
    let events = query_events(connection, project_slug)?;
    let provider_requests = query_provider_requests(connection, project_slug)?;
    let software_actions = query_software_actions(connection, project_slug)?;
    let agent_sessions = query_agent_sessions(connection, project_slug)?;
    let api_keys = query_api_keys(connection)?;
    let node_states = tasks
        .iter()
        .filter_map(|task| {
            task.node_id.as_ref().map(|node_id| NodeRuntimeState {
                node_id: node_id.clone(),
                task_id: task.id.clone(),
                title: task.title.clone(),
                status: task.status.clone(),
                provider_id: task.provider_id.clone(),
                requires_approval: task.requires_approval,
                updated_at: task.updated_at.clone(),
            })
        })
        .collect::<Vec<_>>();
    let task_estimated_tokens = tasks
        .iter()
        .map(|task| task.cost_estimate_tokens)
        .sum::<u64>();
    let waiting_approval_estimated_tokens = tasks
        .iter()
        .filter(|task| task.status == "WaitingApproval")
        .map(|task| task.cost_estimate_tokens)
        .sum::<u64>();
    let agent_token_used = agent_sessions
        .iter()
        .map(|session| session.token_used)
        .sum::<u64>();
    let agent_token_budget = agent_sessions
        .iter()
        .filter_map(|session| session.token_budget)
        .sum::<u64>();
    let token_total = task_estimated_tokens.max(agent_token_used);
    let budget_remaining = if agent_token_budget == 0 {
        None
    } else {
        Some(agent_token_budget as i64 - agent_token_used as i64)
    };

    let stats = RuntimeSnapshotStats {
        projects: projects.len(),
        workflows: workflows.len(),
        tasks: tasks.len(),
        assets: assets.len(),
        events: events.len(),
        provider_requests: provider_requests.len(),
        software_actions: software_actions.len(),
        agent_sessions: agent_sessions.len(),
        api_keys: api_keys.len(),
        waiting_approval: tasks
            .iter()
            .filter(|task| task.status == "WaitingApproval")
            .count(),
        running: tasks.iter().filter(|task| task.status == "Running").count(),
        failed: tasks.iter().filter(|task| task.status == "Failed").count(),
        task_estimated_tokens,
        waiting_approval_estimated_tokens,
        agent_token_used,
        agent_token_budget,
        token_total,
        budget_remaining,
    };

    Ok(RuntimeSnapshot {
        version: 1,
        generated_at: chrono::Utc::now().to_rfc3339(),
        project_filter: project_slug.map(ToString::to_string),
        stats,
        projects,
        workflows,
        node_states,
        tasks,
        assets,
        events,
        provider_requests,
        software_actions,
        agent_sessions,
        api_keys,
    })
}

fn query_projects(
    connection: &Connection,
    project_slug: Option<&str>,
) -> Result<Vec<ProjectSnapshot>> {
    let mut sql = r#"
        SELECT id, slug, name, description, settings, created_at, updated_at
        FROM projects
    "#
    .to_string();
    if project_slug.is_some() {
        sql.push_str(" WHERE slug = ?1");
    }
    sql.push_str(" ORDER BY updated_at DESC");
    let mut statement = connection.prepare(&sql)?;
    let rows = match project_slug {
        Some(slug) => statement.query_map(params![slug], project_row)?,
        None => statement.query_map([], project_row)?,
    };
    collect_rows(rows)
}

fn query_workflows(
    connection: &Connection,
    project_slug: Option<&str>,
) -> Result<Vec<WorkflowSnapshot>> {
    let mut sql = r#"
        SELECT workflows.id, workflows.project_id, workflows.shot_id, workflows.name,
               workflows.nodes, workflows.connections, workflows.created_at, workflows.updated_at
        FROM workflows
        LEFT JOIN projects ON projects.id = workflows.project_id
    "#
    .to_string();
    if project_slug.is_some() {
        sql.push_str(" WHERE projects.slug = ?1");
    }
    sql.push_str(" ORDER BY workflows.updated_at DESC");
    let mut statement = connection.prepare(&sql)?;
    let rows = match project_slug {
        Some(slug) => statement.query_map(params![slug], workflow_row)?,
        None => statement.query_map([], workflow_row)?,
    };
    collect_rows(rows)
}

fn query_tasks(connection: &Connection, project_slug: Option<&str>) -> Result<Vec<TaskSnapshot>> {
    let mut sql = r#"
        SELECT id, project_slug, node_id, title, status, provider_id, cost_estimate_tokens,
               requires_approval, request_metadata_path, created_at, updated_at
        FROM tasks
    "#
    .to_string();
    if project_slug.is_some() {
        sql.push_str(" WHERE project_slug = ?1");
    }
    sql.push_str(" ORDER BY updated_at DESC");
    let mut statement = connection.prepare(&sql)?;
    let rows = match project_slug {
        Some(slug) => statement.query_map(params![slug], task_row)?,
        None => statement.query_map([], task_row)?,
    };
    collect_rows(rows)
}

fn query_assets(connection: &Connection, project_slug: Option<&str>) -> Result<Vec<AssetSnapshot>> {
    let mut sql = r#"
        SELECT id, project_slug, name, asset_type, local_path, source_node_id, provider_url,
               status, created_at
        FROM assets
    "#
    .to_string();
    if project_slug.is_some() {
        sql.push_str(" WHERE project_slug = ?1");
    }
    sql.push_str(" ORDER BY created_at DESC");
    let mut statement = connection.prepare(&sql)?;
    let rows = match project_slug {
        Some(slug) => statement.query_map(params![slug], asset_row)?,
        None => statement.query_map([], asset_row)?,
    };
    collect_rows(rows)
}

fn query_events(connection: &Connection, project_slug: Option<&str>) -> Result<Vec<EventSnapshot>> {
    let mut sql = r#"
        SELECT id, project_slug, level, message, created_at
        FROM workflow_events
    "#
    .to_string();
    if project_slug.is_some() {
        sql.push_str(" WHERE project_slug = ?1");
    }
    sql.push_str(" ORDER BY created_at DESC");
    let mut statement = connection.prepare(&sql)?;
    let rows = match project_slug {
        Some(slug) => statement.query_map(params![slug], event_row)?,
        None => statement.query_map([], event_row)?,
    };
    collect_rows(rows)
}

fn query_provider_requests(
    connection: &Connection,
    project_slug: Option<&str>,
) -> Result<Vec<ProviderRequestSnapshot>> {
    let mut sql = r#"
        SELECT provider_requests.id, provider_requests.task_id, tasks.project_slug,
               provider_requests.provider_id, provider_requests.request_json,
               provider_requests.response_json, provider_requests.metadata_path,
               provider_requests.created_at
        FROM provider_requests
        LEFT JOIN tasks ON tasks.id = provider_requests.task_id
    "#
    .to_string();
    if project_slug.is_some() {
        sql.push_str(" WHERE tasks.project_slug = ?1");
    }
    sql.push_str(" ORDER BY provider_requests.created_at DESC");
    let mut statement = connection.prepare(&sql)?;
    let rows = match project_slug {
        Some(slug) => statement.query_map(params![slug], provider_request_row)?,
        None => statement.query_map([], provider_request_row)?,
    };
    collect_rows(rows)
}

fn query_software_actions(
    connection: &Connection,
    project_slug: Option<&str>,
) -> Result<Vec<SoftwareActionSnapshot>> {
    let mut sql = r#"
        SELECT software_actions.id, software_actions.task_id, software_actions.adapter_id,
               software_actions.action_kind, software_actions.command_json,
               software_actions.verification_json, software_actions.created_at
        FROM software_actions
        LEFT JOIN tasks ON tasks.id = software_actions.task_id
    "#
    .to_string();
    if project_slug.is_some() {
        sql.push_str(" WHERE tasks.project_slug = ?1");
    }
    sql.push_str(" ORDER BY software_actions.created_at DESC");
    let mut statement = connection.prepare(&sql)?;
    let rows = match project_slug {
        Some(slug) => statement.query_map(params![slug], software_action_row)?,
        None => statement.query_map([], software_action_row)?,
    };
    collect_rows(rows)
}

fn query_agent_sessions(
    connection: &Connection,
    project_slug: Option<&str>,
) -> Result<Vec<AgentSessionSnapshot>> {
    let mut sql = r#"
        SELECT id, project_slug, tools, token_budget, token_used, transcript_path,
               created_at, updated_at
        FROM agent_sessions
    "#
    .to_string();
    if project_slug.is_some() {
        sql.push_str(" WHERE project_slug = ?1");
    }
    sql.push_str(" ORDER BY updated_at DESC");
    let mut statement = connection.prepare(&sql)?;
    let rows = match project_slug {
        Some(slug) => statement.query_map(params![slug], agent_session_row)?,
        None => statement.query_map([], agent_session_row)?,
    };
    collect_rows(rows)
}

pub fn query_api_keys(connection: &Connection) -> Result<Vec<ApiKeySnapshot>> {
    let mut statement = connection.prepare(
        r#"
        SELECT id, provider, service_type, encrypted_key, metadata, created_at, updated_at
        FROM api_keys
        ORDER BY provider ASC, service_type ASC, updated_at DESC
        "#,
    )?;
    let rows = statement.query_map([], api_key_row)?;
    collect_rows(rows)
}

fn collect_rows<T, F>(rows: rusqlite::MappedRows<'_, F>) -> Result<Vec<T>>
where
    F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
{
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .context("collect runtime snapshot rows")
}

fn json_value(text: Option<String>) -> Value {
    text.and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or(Value::Null)
}

fn optional_json_value(text: Option<String>) -> Option<Value> {
    text.and_then(|value| serde_json::from_str(&value).ok())
}

fn project_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectSnapshot> {
    Ok(ProjectSnapshot {
        id: row.get(0)?,
        slug: row.get(1)?,
        name: row.get(2)?,
        status: row.get(3)?,
        settings: json_value(row.get(4)?),
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn workflow_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkflowSnapshot> {
    Ok(WorkflowSnapshot {
        id: row.get(0)?,
        project_id: row.get(1)?,
        shot_id: row.get(2)?,
        name: row.get(3)?,
        nodes: json_value(row.get(4)?),
        connections: json_value(row.get(5)?),
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn task_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskSnapshot> {
    let cost: i64 = row.get(6)?;
    let requires_approval: i64 = row.get(7)?;
    Ok(TaskSnapshot {
        id: row.get(0)?,
        project_slug: row.get(1)?,
        node_id: row.get(2)?,
        title: row.get(3)?,
        status: row.get(4)?,
        provider_id: row.get(5)?,
        cost_estimate_tokens: cost.max(0) as u64,
        requires_approval: requires_approval != 0,
        request_metadata_path: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn asset_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AssetSnapshot> {
    Ok(AssetSnapshot {
        id: row.get(0)?,
        project_slug: row.get(1)?,
        name: row.get(2)?,
        asset_type: row.get(3)?,
        local_path: row.get(4)?,
        source_node_id: row.get(5)?,
        provider_url: row.get(6)?,
        status: row.get(7)?,
        created_at: row.get(8)?,
    })
}

fn event_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EventSnapshot> {
    Ok(EventSnapshot {
        id: row.get(0)?,
        project_slug: row.get(1)?,
        level: row.get(2)?,
        message: row.get(3)?,
        created_at: row.get(4)?,
    })
}

fn software_action_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SoftwareActionSnapshot> {
    Ok(SoftwareActionSnapshot {
        id: row.get(0)?,
        task_id: row.get(1)?,
        adapter_id: row.get(2)?,
        action_kind: row.get(3)?,
        command: json_value(row.get(4)?),
        verification: optional_json_value(row.get(5)?),
        created_at: row.get(6)?,
    })
}

fn provider_request_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProviderRequestSnapshot> {
    Ok(ProviderRequestSnapshot {
        id: row.get(0)?,
        task_id: row.get(1)?,
        project_slug: row.get(2)?,
        provider_id: row.get(3)?,
        request: json_value(row.get(4)?),
        response: optional_json_value(row.get(5)?),
        metadata_path: row.get(6)?,
        created_at: row.get(7)?,
    })
}

fn agent_session_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentSessionSnapshot> {
    let token_budget: Option<i64> = row.get(3)?;
    let token_used: i64 = row.get(4)?;
    Ok(AgentSessionSnapshot {
        id: row.get(0)?,
        project_slug: row.get(1)?,
        tools: json_value(row.get(2)?),
        token_budget: token_budget.map(|value| value.max(0) as u64),
        token_used: token_used.max(0) as u64,
        transcript_path: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn api_key_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ApiKeySnapshot> {
    let key_material: String = row.get(3)?;
    let metadata = json_value(row.get(4)?);
    Ok(ApiKeySnapshot {
        id: row.get(0)?,
        provider: row.get(1)?,
        service_type: row.get(2)?,
        configured: !key_material.trim().is_empty(),
        key_hint: key_hint(&metadata, &key_material),
        metadata,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn key_hint(metadata: &Value, secret: &str) -> Option<String> {
    if let Some(hint) = metadata
        .pointer("/credential/key_hint")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        return Some(hint.to_string());
    }
    if secret.starts_with("pool:v1:aes256gcm:") {
        return Some("encrypted".to_string());
    }
    let trimmed = secret.trim();
    if trimmed.is_empty() {
        return None;
    }
    let suffix = trimmed
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    Some(format!("...{suffix}"))
}
