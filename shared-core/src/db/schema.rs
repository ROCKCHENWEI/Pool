pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT,
    settings TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS shots (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    name TEXT,
    position INTEGER DEFAULT 0,
    duration REAL DEFAULT 0,
    status TEXT DEFAULT 'idle',
    workflow_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS workflows (
    id TEXT PRIMARY KEY,
    project_id TEXT,
    shot_id TEXT,
    name TEXT,
    nodes TEXT NOT NULL,
    connections TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (shot_id) REFERENCES shots(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY,
    project_slug TEXT NOT NULL,
    node_id TEXT,
    title TEXT NOT NULL,
    status TEXT NOT NULL,
    provider_id TEXT,
    cost_estimate_tokens INTEGER DEFAULT 0,
    requires_approval INTEGER DEFAULT 0,
    request_metadata_path TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS assets (
    id TEXT PRIMARY KEY,
    project_slug TEXT NOT NULL,
    name TEXT NOT NULL,
    asset_type TEXT NOT NULL,
    local_path TEXT NOT NULL,
    source_node_id TEXT,
    provider_url TEXT,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS provider_requests (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    request_json TEXT NOT NULL,
    response_json TEXT,
    metadata_path TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS workflow_events (
    id TEXT PRIMARY KEY,
    project_slug TEXT NOT NULL,
    level TEXT NOT NULL,
    message TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS software_actions (
    id TEXT PRIMARY KEY,
    task_id TEXT,
    adapter_id TEXT NOT NULL,
    action_kind TEXT NOT NULL,
    command_json TEXT NOT NULL,
    verification_json TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS agent_sessions (
    id TEXT PRIMARY KEY,
    project_slug TEXT NOT NULL,
    tools TEXT NOT NULL,
    token_budget INTEGER,
    token_used INTEGER DEFAULT 0,
    transcript_path TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS embeddings (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    type TEXT NOT NULL,
    vector BLOB,
    metadata TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS api_keys (
    id TEXT PRIMARY KEY,
    provider TEXT NOT NULL,
    service_type TEXT NOT NULL,
    encrypted_key TEXT NOT NULL,
    metadata TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_tasks_project ON tasks(project_slug);
CREATE INDEX IF NOT EXISTS idx_assets_project ON assets(project_slug);
CREATE INDEX IF NOT EXISTS idx_provider_requests_task ON provider_requests(task_id);
CREATE INDEX IF NOT EXISTS idx_workflow_events_project ON workflow_events(project_slug);
CREATE UNIQUE INDEX IF NOT EXISTS idx_api_keys_provider_service ON api_keys(provider, service_type);
"#;

#[cfg(test)]
mod tests {
    use super::SCHEMA;

    #[test]
    fn schema_contains_runtime_tables_from_framework_plan() {
        for table in [
            "projects",
            "shots",
            "workflows",
            "tasks",
            "assets",
            "provider_requests",
            "workflow_events",
            "software_actions",
            "agent_sessions",
            "embeddings",
            "api_keys",
        ] {
            assert!(
                SCHEMA.contains(&format!("CREATE TABLE IF NOT EXISTS {table}")),
                "missing table: {table}"
            );
        }
    }
}
