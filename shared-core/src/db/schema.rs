//! Database schema definitions
//!
//! Contains SQL schema for all database tables.

pub const SCHEMA: &str = r#"
-- Projects table
CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    settings TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Shots table
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

-- Workflows table
CREATE TABLE IF NOT EXISTS workflows (
    id TEXT PRIMARY KEY,
    shot_id TEXT,
    name TEXT,
    nodes TEXT NOT NULL,
    connections TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (shot_id) REFERENCES shots(id) ON DELETE CASCADE
);

-- Embeddings table
CREATE TABLE IF NOT EXISTS embeddings (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    type TEXT NOT NULL,
    vector BLOB,
    metadata TEXT,
    created_at TEXT NOT NULL
);

-- API Keys table (encrypted storage)
CREATE TABLE IF NOT EXISTS api_keys (
    id TEXT PRIMARY KEY,
    provider TEXT NOT NULL,
    service_type TEXT NOT NULL,
    encrypted_key TEXT NOT NULL,
    metadata TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Render tasks table
CREATE TABLE IF NOT EXISTS render_tasks (
    id TEXT PRIMARY KEY,
    shot_id TEXT NOT NULL,
    status TEXT DEFAULT 'pending',
    progress REAL DEFAULT 0,
    started_at TEXT,
    completed_at TEXT,
    output_path TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (shot_id) REFERENCES shots(id) ON DELETE CASCADE
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_shots_project ON shots(project_id);
CREATE INDEX IF NOT EXISTS idx_workflows_shot ON workflows(shot_id);
CREATE INDEX IF NOT EXISTS idx_render_tasks_shot ON render_tasks(shot_id);
"#;
