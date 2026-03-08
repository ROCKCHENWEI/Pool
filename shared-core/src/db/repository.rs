//! Database repository implementation
//!
//! Provides CRUD operations for all data models using the repository pattern.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

use crate::models::{Project, Shot, ShotStatus, Workflow};

use super::SCHEMA;

/// Main database handle providing access to all repository operations
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Create a new database connection and initialize schema
    ///
    /// # Arguments
    /// * `database_url` - SQLite connection string (e.g., ":memory:" or "file.db")
    ///
    /// # Example
    /// ```ignore
    /// let db = Database::new(":memory:").await?;
    /// ```
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
            .context("Failed to connect to database")?;

        // Run migrations
        sqlx::raw_sql(SCHEMA)
            .execute(&pool)
            .await
            .context("Failed to run schema migrations")?;

        Ok(Self { pool })
    }

    /// Check if the database connection is healthy
    pub async fn is_healthy(&self) -> bool {
        sqlx::query("SELECT 1").fetch_one(&self.pool).await.is_ok()
    }

    // =========================================================================
    // Project Operations
    // =========================================================================

    /// Create a new project in the database
    pub async fn create_project(&self, project: &Project) -> Result<()> {
        sqlx::query(
            "INSERT INTO projects (id, name, description, settings, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&project.id)
        .bind(&project.name)
        .bind(&project.description)
        .bind(serde_json::to_string(&serde_json::Value::Null)?)
        .bind(project.created_at.to_rfc3339())
        .bind(project.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .context("Failed to create project")?;

        Ok(())
    }

    /// Get a project by ID
    pub async fn get_project(&self, id: &str) -> Result<Project> {
        let row: (String, String, Option<String>, String, String, String) = sqlx::query_as(
            "SELECT id, name, description, settings, created_at, updated_at
             FROM projects WHERE id = ?",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to get project")?;

        Ok(Project {
            id: row.0,
            name: row.1,
            description: row.2,
            shots: vec![], // Load separately
            created_at: DateTime::parse_from_rfc3339(&row.4)
                .context("Failed to parse created_at")?
                .with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339(&row.5)
                .context("Failed to parse updated_at")?
                .with_timezone(&Utc),
        })
    }

    /// Update an existing project
    pub async fn update_project(&self, project: &Project) -> Result<()> {
        let now = Utc::now();
        sqlx::query("UPDATE projects SET name = ?, description = ?, updated_at = ? WHERE id = ?")
            .bind(&project.name)
            .bind(&project.description)
            .bind(now.to_rfc3339())
            .bind(&project.id)
            .execute(&self.pool)
            .await
            .context("Failed to update project")?;

        Ok(())
    }

    /// Delete a project by ID
    pub async fn delete_project(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM projects WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .context("Failed to delete project")?;

        Ok(())
    }

    /// List all projects
    pub async fn list_projects(&self) -> Result<Vec<Project>> {
        let rows: Vec<(String, String, Option<String>, String, String, String)> =
            sqlx::query_as(
                "SELECT id, name, description, settings, created_at, updated_at
                 FROM projects ORDER BY created_at DESC",
            )
            .fetch_all(&self.pool)
            .await
            .context("Failed to list projects")?;

        let projects = rows
            .into_iter()
            .map(|row| {
                Ok(Project {
                    id: row.0,
                    name: row.1,
                    description: row.2,
                    shots: vec![],
                    created_at: DateTime::parse_from_rfc3339(&row.4)
                        .context("Failed to parse created_at")?
                        .with_timezone(&Utc),
                    updated_at: DateTime::parse_from_rfc3339(&row.5)
                        .context("Failed to parse updated_at")?
                        .with_timezone(&Utc),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(projects)
    }

    // =========================================================================
    // Shot Operations
    // =========================================================================

    /// Create a new shot in the database
    pub async fn create_shot(&self, shot: &Shot) -> Result<()> {
        let status_str = match &shot.status {
            ShotStatus::Idle => "idle",
            ShotStatus::Pending => "pending",
            ShotStatus::Processing => "processing",
            ShotStatus::Completed => "completed",
            ShotStatus::Failed => "failed",
        };

        let workflow_id = shot.workflow.as_ref().map(|w| w.id.clone());

        sqlx::query(
            "INSERT INTO shots (id, project_id, name, position, duration, status, workflow_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&shot.id)
        .bind(&shot.project_id)
        .bind(&shot.name)
        .bind(shot.position)
        .bind(shot.duration)
        .bind(status_str)
        .bind(workflow_id)
        .bind(shot.created_at.to_rfc3339())
        .bind(shot.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .context("Failed to create shot")?;

        Ok(())
    }

    /// Get a shot by ID
    pub async fn get_shot(&self, id: &str) -> Result<Shot> {
        let row: (String, String, String, i32, f64, String, Option<String>, String, String) =
            sqlx::query_as(
                "SELECT id, project_id, name, position, duration, status, workflow_id, created_at, updated_at
                 FROM shots WHERE id = ?",
            )
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .context("Failed to get shot")?;

        let status = match row.5.as_str() {
            "idle" => ShotStatus::Idle,
            "pending" => ShotStatus::Pending,
            "processing" => ShotStatus::Processing,
            "completed" => ShotStatus::Completed,
            "failed" => ShotStatus::Failed,
            _ => ShotStatus::Idle,
        };

        Ok(Shot {
            id: row.0,
            project_id: row.1,
            name: row.2,
            position: row.3,
            duration: row.4,
            workflow: None, // Load separately if needed
            status,
            created_at: DateTime::parse_from_rfc3339(&row.7)
                .context("Failed to parse created_at")?
                .with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339(&row.8)
                .context("Failed to parse updated_at")?
                .with_timezone(&Utc),
        })
    }

    /// Update an existing shot
    pub async fn update_shot(&self, shot: &Shot) -> Result<()> {
        let status_str = match &shot.status {
            ShotStatus::Idle => "idle",
            ShotStatus::Pending => "pending",
            ShotStatus::Processing => "processing",
            ShotStatus::Completed => "completed",
            ShotStatus::Failed => "failed",
        };

        let workflow_id = shot.workflow.as_ref().map(|w| w.id.clone());

        let now = Utc::now();
        sqlx::query(
            "UPDATE shots SET name = ?, position = ?, duration = ?, status = ?, workflow_id = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&shot.name)
        .bind(shot.position)
        .bind(shot.duration)
        .bind(status_str)
        .bind(workflow_id)
        .bind(now.to_rfc3339())
        .bind(&shot.id)
        .execute(&self.pool)
        .await
        .context("Failed to update shot")?;

        Ok(())
    }

    /// Delete a shot by ID
    pub async fn delete_shot(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM shots WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .context("Failed to delete shot")?;

        Ok(())
    }

    /// List all shots for a project
    pub async fn list_shots_by_project(&self, project_id: &str) -> Result<Vec<Shot>> {
        let rows: Vec<(String, String, String, i32, f64, String, Option<String>, String, String)> =
            sqlx::query_as(
                "SELECT id, project_id, name, position, duration, status, workflow_id, created_at, updated_at
                 FROM shots WHERE project_id = ? ORDER BY position",
            )
            .bind(project_id)
            .fetch_all(&self.pool)
            .await
            .context("Failed to list shots")?;

        let shots = rows
            .into_iter()
            .map(|row| {
                let status = match row.5.as_str() {
                    "idle" => ShotStatus::Idle,
                    "pending" => ShotStatus::Pending,
                    "processing" => ShotStatus::Processing,
                    "completed" => ShotStatus::Completed,
                    "failed" => ShotStatus::Failed,
                    _ => ShotStatus::Idle,
                };

                Ok(Shot {
                    id: row.0,
                    project_id: row.1,
                    name: row.2,
                    position: row.3,
                    duration: row.4,
                    workflow: None,
                    status,
                    created_at: DateTime::parse_from_rfc3339(&row.7)
                        .context("Failed to parse created_at")?
                        .with_timezone(&Utc),
                    updated_at: DateTime::parse_from_rfc3339(&row.8)
                        .context("Failed to parse updated_at")?
                        .with_timezone(&Utc),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(shots)
    }

    // =========================================================================
    // Workflow Operations
    // =========================================================================

    /// Create a new workflow in the database
    pub async fn create_workflow(&self, workflow: &Workflow) -> Result<()> {
        let nodes_json = serde_json::to_string(&workflow.nodes)?;
        let connections_json = serde_json::to_string(&workflow.connections)?;

        sqlx::query(
            "INSERT INTO workflows (id, shot_id, name, nodes, connections, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&workflow.id)
        .bind(&workflow.shot_id)
        .bind(&workflow.name)
        .bind(nodes_json)
        .bind(connections_json)
        .bind(workflow.created_at.to_rfc3339())
        .bind(workflow.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .context("Failed to create workflow")?;

        Ok(())
    }

    /// Get a workflow by ID
    pub async fn get_workflow(&self, id: &str) -> Result<Workflow> {
        let row: (String, Option<String>, String, String, Option<String>, String, String) =
            sqlx::query_as(
                "SELECT id, shot_id, name, nodes, connections, created_at, updated_at
                 FROM workflows WHERE id = ?",
            )
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .context("Failed to get workflow")?;

        use crate::models::{Connection, Node};

        let nodes: Vec<Node> =
            serde_json::from_str(&row.3).context("Failed to parse nodes JSON")?;
        let connections: Vec<Connection> = row
            .4
            .map(|s| serde_json::from_str(&s))
            .transpose()
            .context("Failed to parse connections JSON")?
            .unwrap_or_default();

        Ok(Workflow {
            id: row.0,
            shot_id: row.1.unwrap_or_default(),
            name: row.2,
            nodes,
            connections,
            created_at: DateTime::parse_from_rfc3339(&row.5)
                .context("Failed to parse created_at")?
                .with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339(&row.6)
                .context("Failed to parse updated_at")?
                .with_timezone(&Utc),
        })
    }

    /// Get workflow by shot ID
    pub async fn get_workflow_by_shot(&self, shot_id: &str) -> Result<Option<Workflow>> {
        let row_result: Option<(
            String,
            Option<String>,
            String,
            String,
            Option<String>,
            String,
            String,
        )> = sqlx::query_as(
            "SELECT id, shot_id, name, nodes, connections, created_at, updated_at
             FROM workflows WHERE shot_id = ?",
        )
        .bind(shot_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get workflow by shot")?;

        if let Some(row) = row_result {
            use crate::models::{Connection, Node};

            let nodes: Vec<Node> =
                serde_json::from_str(&row.3).context("Failed to parse nodes JSON")?;
            let connections: Vec<Connection> = row
                .4
                .map(|s| serde_json::from_str(&s))
                .transpose()
                .context("Failed to parse connections JSON")?
                .unwrap_or_default();

            Ok(Some(Workflow {
                id: row.0,
                shot_id: row.1.unwrap_or_default(),
                name: row.2,
                nodes,
                connections,
                created_at: DateTime::parse_from_rfc3339(&row.5)
                    .context("Failed to parse created_at")?
                    .with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&row.6)
                    .context("Failed to parse updated_at")?
                    .with_timezone(&Utc),
            }))
        } else {
            Ok(None)
        }
    }

    /// Update an existing workflow
    pub async fn update_workflow(&self, workflow: &Workflow) -> Result<()> {
        let nodes_json = serde_json::to_string(&workflow.nodes)?;
        let connections_json = serde_json::to_string(&workflow.connections)?;

        let now = Utc::now();
        sqlx::query(
            "UPDATE workflows SET name = ?, nodes = ?, connections = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&workflow.name)
        .bind(nodes_json)
        .bind(connections_json)
        .bind(now.to_rfc3339())
        .bind(&workflow.id)
        .execute(&self.pool)
        .await
        .context("Failed to update workflow")?;

        Ok(())
    }

    /// Delete a workflow by ID
    pub async fn delete_workflow(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM workflows WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .context("Failed to delete workflow")?;

        Ok(())
    }
}
