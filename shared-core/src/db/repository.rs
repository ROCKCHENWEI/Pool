use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose, Engine as _};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;
use serde_json::{json as serde_json_value, Value};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::process::Command;

use crate::assets::build_asset_records;
use crate::control::{SoftwareActionResult, SoftwareControlAction};
use crate::db::snapshot::{
    build_runtime_snapshot, query_api_keys, ApiKeySnapshot, RuntimeSnapshot, TaskSnapshot,
};
use crate::db::SCHEMA;
use crate::engine::PoolRuntimePlan;
use crate::models::{
    AgentSession, AssetRecord, Project, RuntimeEvent, RuntimeEventLevel, RuntimeTask, Shot,
    TaskStatus,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeRepositoryStats {
    pub projects: u64,
    pub shots: u64,
    pub workflows: u64,
    pub tasks: u64,
    pub assets: u64,
    pub events: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderRequestRecord {
    pub id: String,
    pub task_id: String,
    pub provider_id: String,
    pub request_json: Value,
    pub response_json: Option<Value>,
    pub metadata_path: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SoftwareActionRecord {
    pub id: String,
    pub task_id: Option<String>,
    pub adapter_id: String,
    pub action_kind: String,
    pub command_json: Value,
    pub verification_json: Option<Value>,
    pub created_at: String,
}

pub struct RuntimeRepository {
    connection: Connection,
}

impl RuntimeRepository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let connection = Connection::open(path).context("open Pool runtime SQLite database")?;
        let repository = Self { connection };
        repository.enable_foreign_keys()?;
        Ok(repository)
    }

    pub fn in_memory() -> Result<Self> {
        let connection = Connection::open_in_memory().context("open in-memory Pool runtime DB")?;
        let repository = Self { connection };
        repository.enable_foreign_keys()?;
        Ok(repository)
    }

    pub fn migrate(&self) -> Result<()> {
        self.connection
            .execute_batch(SCHEMA)
            .context("run Pool runtime schema migration")
    }

    pub fn persist_plan(&self, plan: &PoolRuntimePlan) -> Result<RuntimeRepositoryStats> {
        self.insert_project(&plan.project)?;
        self.insert_workflow(&plan.project, plan)?;

        for shot in &plan.shots {
            self.insert_shot(&plan.project, shot, Some(&plan.workflow.id))?;
        }

        for node in plan.workflow.nodes.values() {
            let mut task = RuntimeTask::new(plan.project.slug.clone(), node.title.clone());
            task.node_id = Some(node.id.clone());
            task.provider_id = node
                .provider_id
                .clone()
                .or_else(|| node.software_adapter_id.clone());
            task.cost_estimate_tokens = node.cost_estimate_tokens;
            task.requires_approval = node.requires_approval;
            task.status = if node.requires_approval {
                crate::models::TaskStatus::WaitingApproval
            } else {
                crate::models::TaskStatus::Ready
            };
            self.insert_task(&task)?;
        }

        self.insert_event(&RuntimeEvent::new(
            plan.project.slug.clone(),
            RuntimeEventLevel::Info,
            format!("persisted runtime plan: {}", plan.workflow.title),
        ))?;

        self.stats()
    }

    pub fn insert_project(&self, project: &Project) -> Result<()> {
        self.connection.execute(
            r#"
            INSERT OR REPLACE INTO projects
                (id, slug, name, description, settings, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                project.id,
                project.slug,
                project.title,
                enum_text(&project.status)?,
                json(&project.output_targets)?,
                project.created_at.to_rfc3339(),
                project.updated_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn insert_shot(
        &self,
        project: &Project,
        shot: &Shot,
        workflow_id: Option<&str>,
    ) -> Result<()> {
        self.connection.execute(
            r#"
            INSERT OR REPLACE INTO shots
                (id, project_id, name, position, duration, status, workflow_id, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            params![
                shot.id,
                project.id,
                shot.title,
                shot.timeline_start_ms as i64,
                shot.duration_ms as f64 / 1000.0,
                enum_text(&shot.status)?,
                workflow_id,
                shot.created_at.to_rfc3339(),
                shot.updated_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn insert_workflow(&self, project: &Project, plan: &PoolRuntimePlan) -> Result<()> {
        self.connection.execute(
            r#"
            INSERT OR REPLACE INTO workflows
                (id, project_id, shot_id, name, nodes, connections, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                plan.workflow.id,
                project.id,
                Option::<String>::None,
                plan.workflow.title,
                json(&plan.workflow.nodes)?,
                json(&plan.workflow.connections)?,
                plan.workflow.created_at.to_rfc3339(),
                plan.workflow.updated_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn insert_task(&self, task: &RuntimeTask) -> Result<()> {
        self.connection.execute(
            r#"
            INSERT INTO tasks
                (id, project_slug, node_id, title, status, provider_id, cost_estimate_tokens,
                 requires_approval, request_metadata_path, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(id) DO UPDATE SET
                project_slug = excluded.project_slug,
                node_id = excluded.node_id,
                title = excluded.title,
                status = excluded.status,
                provider_id = excluded.provider_id,
                cost_estimate_tokens = excluded.cost_estimate_tokens,
                requires_approval = excluded.requires_approval,
                request_metadata_path = excluded.request_metadata_path,
                updated_at = excluded.updated_at
            "#,
            params![
                task.id,
                task.project_slug,
                task.node_id,
                task.title,
                enum_text(&task.status)?,
                task.provider_id,
                task.cost_estimate_tokens as i64,
                task.requires_approval as i64,
                task.request_metadata_path,
                task.created_at.to_rfc3339(),
                task.updated_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn update_task_status(&self, task_id: &str, status: TaskStatus) -> Result<()> {
        self.connection.execute(
            r#"
            UPDATE tasks
            SET status = ?1, updated_at = ?2
            WHERE id = ?3
            "#,
            params![
                enum_text(&status)?,
                chrono::Utc::now().to_rfc3339(),
                task_id
            ],
        )?;
        Ok(())
    }

    pub fn update_task_node_id(&self, task_id: &str, node_id: Option<&str>) -> Result<()> {
        self.connection.execute(
            r#"
            UPDATE tasks
            SET node_id = ?1, updated_at = ?2
            WHERE id = ?3
            "#,
            params![node_id, chrono::Utc::now().to_rfc3339(), task_id],
        )?;
        Ok(())
    }

    pub fn update_task_request_metadata_path(
        &self,
        task_id: &str,
        request_metadata_path: Option<&str>,
    ) -> Result<()> {
        self.connection.execute(
            r#"
            UPDATE tasks
            SET request_metadata_path = ?1, updated_at = ?2
            WHERE id = ?3
            "#,
            params![
                request_metadata_path,
                chrono::Utc::now().to_rfc3339(),
                task_id
            ],
        )?;
        Ok(())
    }

    pub fn approve_task(&self, task_id: &str) -> Result<TaskSnapshot> {
        let task = self.task_snapshot(task_id)?;
        if task.status != "WaitingApproval" {
            anyhow::bail!("task is not waiting for approval: {task_id}");
        }

        self.update_task_status(task_id, TaskStatus::Ready)?;
        self.insert_event(&RuntimeEvent::new(
            task.project_slug.clone(),
            RuntimeEventLevel::Ok,
            format!("approved task: {}", task.title),
        ))?;
        self.task_snapshot(task_id)
    }

    pub fn task_snapshot(&self, task_id: &str) -> Result<TaskSnapshot> {
        self.connection
            .query_row(
                r#"
                SELECT id, project_slug, node_id, title, status, provider_id,
                       cost_estimate_tokens, requires_approval, request_metadata_path,
                       created_at, updated_at
                FROM tasks
                WHERE id = ?1
                "#,
                params![task_id],
                |row| {
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
                },
            )
            .optional()?
            .ok_or_else(|| anyhow::anyhow!("task not found: {task_id}"))
    }

    pub fn runtime_task(&self, task_id: &str) -> Result<RuntimeTask> {
        self.connection
            .query_row(
                r#"
                SELECT id, project_slug, node_id, title, status, provider_id,
                       cost_estimate_tokens, requires_approval, request_metadata_path,
                       created_at, updated_at
                FROM tasks
                WHERE id = ?1
                "#,
                params![task_id],
                |row| {
                    let status: String = row.get(4)?;
                    let cost: i64 = row.get(6)?;
                    let requires_approval: i64 = row.get(7)?;
                    let created_at: String = row.get(9)?;
                    let updated_at: String = row.get(10)?;
                    Ok(RuntimeTask {
                        id: row.get(0)?,
                        project_slug: row.get(1)?,
                        node_id: row.get(2)?,
                        title: row.get(3)?,
                        status: task_status_from_text(&status)?,
                        provider_id: row.get(5)?,
                        cost_estimate_tokens: cost.max(0) as u64,
                        requires_approval: requires_approval != 0,
                        request_metadata_path: row.get(8)?,
                        created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
                            .map_err(|error| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    9,
                                    rusqlite::types::Type::Text,
                                    Box::new(error),
                                )
                            })?
                            .with_timezone(&chrono::Utc),
                        updated_at: chrono::DateTime::parse_from_rfc3339(&updated_at)
                            .map_err(|error| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    10,
                                    rusqlite::types::Type::Text,
                                    Box::new(error),
                                )
                            })?
                            .with_timezone(&chrono::Utc),
                    })
                },
            )
            .optional()?
            .ok_or_else(|| anyhow::anyhow!("task not found: {task_id}"))
    }

    pub fn insert_provider_request(
        &self,
        task_id: &str,
        provider_id: &str,
        request_json: &Value,
        metadata_path: Option<&str>,
    ) -> Result<ProviderRequestRecord> {
        let id = uuid::Uuid::new_v4().to_string();
        let created_at = chrono::Utc::now().to_rfc3339();
        self.connection.execute(
            r#"
            INSERT INTO provider_requests
                (id, task_id, provider_id, request_json, response_json, metadata_path, created_at)
            VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6)
            "#,
            params![
                id,
                task_id,
                provider_id,
                json(request_json)?,
                metadata_path,
                created_at
            ],
        )?;
        Ok(ProviderRequestRecord {
            id,
            task_id: task_id.to_string(),
            provider_id: provider_id.to_string(),
            request_json: request_json.clone(),
            response_json: None,
            metadata_path: metadata_path.map(ToString::to_string),
            created_at,
        })
    }

    pub fn update_provider_request_response(
        &self,
        request_id: &str,
        response_json: &Value,
        metadata_path: Option<&str>,
    ) -> Result<()> {
        self.connection.execute(
            r#"
            UPDATE provider_requests
            SET response_json = ?1,
                metadata_path = COALESCE(?2, metadata_path)
            WHERE id = ?3
            "#,
            params![json(response_json)?, metadata_path, request_id],
        )?;
        Ok(())
    }

    pub fn latest_provider_request(&self, task_id: &str) -> Result<Option<ProviderRequestRecord>> {
        self.connection
            .query_row(
                r#"
                SELECT id, task_id, provider_id, request_json, response_json, metadata_path, created_at
                FROM provider_requests
                WHERE task_id = ?1
                ORDER BY created_at DESC
                LIMIT 1
                "#,
                params![task_id],
                provider_request_record_from_row,
            )
            .optional()
            .context("read latest provider request")
    }

    pub fn latest_software_action(&self, task_id: &str) -> Result<Option<SoftwareActionRecord>> {
        self.connection
            .query_row(
                r#"
                SELECT id, task_id, adapter_id, action_kind, command_json, verification_json, created_at
                FROM software_actions
                WHERE task_id = ?1
                ORDER BY created_at DESC
                LIMIT 1
                "#,
                params![task_id],
                software_action_record_from_row,
            )
            .optional()
            .context("read latest software action")
    }

    pub fn insert_asset(&self, asset: &AssetRecord) -> Result<()> {
        self.connection.execute(
            r#"
            INSERT OR REPLACE INTO assets
                (id, project_slug, name, asset_type, local_path, source_node_id, provider_url, status, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            params![
                asset.id,
                asset.project_slug,
                asset.name,
                asset.asset_type,
                asset.local_path,
                asset.source_node_id,
                asset.provider_url,
                enum_text(&asset.status)?,
                asset.created_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn insert_assets(&self, assets: &[AssetRecord]) -> Result<()> {
        for asset in assets {
            self.insert_asset(asset)?;
        }
        Ok(())
    }

    pub fn index_local_outputs(
        &self,
        project_slug: &str,
        source_node_id: Option<&str>,
        provider_url: Option<&str>,
        local_paths: &[String],
    ) -> Result<Vec<AssetRecord>> {
        let assets = build_asset_records(project_slug, source_node_id, provider_url, local_paths);
        self.insert_assets(&assets)?;
        Ok(assets)
    }

    pub fn insert_event(&self, event: &RuntimeEvent) -> Result<()> {
        self.connection.execute(
            r#"
            INSERT OR REPLACE INTO workflow_events
                (id, project_slug, level, message, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![
                event.id,
                event.project_slug,
                enum_text(&event.level)?,
                event.message,
                event.created_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn insert_software_action(
        &self,
        action_id: &str,
        task_id: Option<&str>,
        action: &SoftwareControlAction,
        result: Option<&SoftwareActionResult>,
    ) -> Result<()> {
        let verification_json = result
            .map(json)
            .transpose()
            .context("serialize software action result")?;
        self.connection.execute(
            r#"
            INSERT OR REPLACE INTO software_actions
                (id, task_id, adapter_id, action_kind, command_json, verification_json, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                action_id,
                task_id,
                action.adapter_id,
                enum_text(&action.action_kind)?,
                json(action)?,
                verification_json,
                chrono::Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn update_software_action_verification(
        &self,
        action_id: &str,
        verification: Value,
    ) -> Result<()> {
        let rows = self.connection.execute(
            r#"
            UPDATE software_actions
            SET verification_json = ?1
            WHERE id = ?2
            "#,
            params![json(&verification)?, action_id],
        )?;
        if rows == 0 {
            bail!("software action not found: {action_id}");
        }
        Ok(())
    }

    pub fn insert_agent_session(&self, session: &AgentSession) -> Result<()> {
        self.connection.execute(
            r#"
            INSERT OR REPLACE INTO agent_sessions
                (id, project_slug, tools, token_budget, token_used, transcript_path, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                session.id,
                session.project_slug,
                json(&session.tools)?,
                session.token_budget.map(|value| value as i64),
                session.token_used as i64,
                session.transcript_path,
                session.created_at.to_rfc3339(),
                session.updated_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn upsert_api_key(
        &self,
        provider: &str,
        service_type: &str,
        key_material: &str,
        metadata: Value,
    ) -> Result<ApiKeySnapshot> {
        self.upsert_api_key_with_codec(
            provider,
            service_type,
            key_material,
            metadata,
            &CredentialCodec::from_env(),
        )
    }

    fn upsert_api_key_with_codec(
        &self,
        provider: &str,
        service_type: &str,
        key_material: &str,
        metadata: Value,
        codec: &CredentialCodec,
    ) -> Result<ApiKeySnapshot> {
        let provider = provider.trim();
        let service_type = service_type.trim();
        let key_material = key_material.trim();
        if provider.is_empty() {
            anyhow::bail!("api key provider cannot be empty");
        }
        if service_type.is_empty() {
            anyhow::bail!("api key service_type cannot be empty");
        }
        if key_material.is_empty() {
            anyhow::bail!("api key cannot be empty");
        }

        let now = chrono::Utc::now().to_rfc3339();
        let stored = codec.seal(provider, service_type, key_material)?;
        let metadata = merge_credential_metadata(metadata, &stored)?;
        let existing = self
            .connection
            .query_row(
                "SELECT id, created_at FROM api_keys WHERE provider = ?1 AND service_type = ?2",
                params![provider, service_type],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let (id, created_at) =
            existing.unwrap_or_else(|| (uuid::Uuid::new_v4().to_string(), now.clone()));

        self.connection.execute(
            r#"
            INSERT OR REPLACE INTO api_keys
                (id, provider, service_type, encrypted_key, metadata, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                id,
                provider,
                service_type,
                stored.key_material,
                serde_json::to_string(&metadata).context("serialize api key metadata")?,
                created_at,
                now
            ],
        )?;
        self.api_key_snapshot(provider, service_type)
    }

    pub fn api_key_secret(&self, provider: &str, service_type: &str) -> Result<Option<String>> {
        self.api_key_secret_with_codec(provider, service_type, &CredentialCodec::from_env())
    }

    fn api_key_secret_with_codec(
        &self,
        provider: &str,
        service_type: &str,
        codec: &CredentialCodec,
    ) -> Result<Option<String>> {
        self.connection
            .query_row(
                "SELECT encrypted_key FROM api_keys WHERE provider = ?1 AND service_type = ?2",
                params![provider, service_type],
                |row| row.get(0),
            )
            .optional()
            .context("read api key secret")
            .and_then(|value: Option<String>| {
                value
                    .map(|stored| codec.open(&stored, provider, service_type))
                    .transpose()
            })
    }

    pub fn api_key_snapshot(&self, provider: &str, service_type: &str) -> Result<ApiKeySnapshot> {
        let keys = query_api_keys(&self.connection)?;
        keys.into_iter()
            .find(|key| key.provider == provider && key.service_type == service_type)
            .ok_or_else(|| anyhow::anyhow!("api key not found: {provider}/{service_type}"))
    }

    pub fn api_key_snapshots(&self) -> Result<Vec<ApiKeySnapshot>> {
        query_api_keys(&self.connection)
    }

    pub fn table_count(&self, table: &str) -> Result<u64> {
        if !table
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            anyhow::bail!("unsafe table name: {table}");
        }
        let sql = format!("SELECT COUNT(*) FROM {table}");
        let count: i64 = self.connection.query_row(&sql, [], |row| row.get(0))?;
        Ok(count as u64)
    }

    pub fn stats(&self) -> Result<RuntimeRepositoryStats> {
        Ok(RuntimeRepositoryStats {
            projects: self.table_count("projects")?,
            shots: self.table_count("shots")?,
            workflows: self.table_count("workflows")?,
            tasks: self.table_count("tasks")?,
            assets: self.table_count("assets")?,
            events: self.table_count("workflow_events")?,
        })
    }

    pub fn snapshot(&self, project_slug: Option<&str>) -> Result<RuntimeSnapshot> {
        build_runtime_snapshot(&self.connection, project_slug)
    }

    fn enable_foreign_keys(&self) -> Result<()> {
        self.connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .context("enable SQLite foreign keys")
    }
}

fn json(value: impl Serialize) -> Result<String> {
    serde_json::to_string(&value).context("serialize repository JSON field")
}

fn enum_text(value: impl Serialize) -> Result<String> {
    match serde_json::to_value(value).context("serialize enum text")? {
        Value::String(text) => Ok(text),
        other => Ok(other.to_string()),
    }
}

fn provider_request_record_from_row(row: &Row<'_>) -> rusqlite::Result<ProviderRequestRecord> {
    let request_raw: String = row.get(3)?;
    let response_raw: Option<String> = row.get(4)?;
    let request_json = serde_json::from_str(&request_raw).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(error))
    })?;
    let response_json = response_raw
        .map(|raw| {
            serde_json::from_str(&raw).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    4,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })
        })
        .transpose()?;

    Ok(ProviderRequestRecord {
        id: row.get(0)?,
        task_id: row.get(1)?,
        provider_id: row.get(2)?,
        request_json,
        response_json,
        metadata_path: row.get(5)?,
        created_at: row.get(6)?,
    })
}

fn software_action_record_from_row(row: &Row<'_>) -> rusqlite::Result<SoftwareActionRecord> {
    let command_raw: String = row.get(4)?;
    let verification_raw: Option<String> = row.get(5)?;
    let command_json = serde_json::from_str(&command_raw).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(error))
    })?;
    let verification_json = verification_raw
        .map(|raw| {
            serde_json::from_str(&raw).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })
        })
        .transpose()?;

    Ok(SoftwareActionRecord {
        id: row.get(0)?,
        task_id: row.get(1)?,
        adapter_id: row.get(2)?,
        action_kind: row.get(3)?,
        command_json,
        verification_json,
        created_at: row.get(6)?,
    })
}

fn task_status_from_text(status: &str) -> rusqlite::Result<TaskStatus> {
    match status {
        "Queued" => Ok(TaskStatus::Queued),
        "Ready" => Ok(TaskStatus::Ready),
        "Running" => Ok(TaskStatus::Running),
        "WaitingApproval" => Ok(TaskStatus::WaitingApproval),
        "Succeeded" => Ok(TaskStatus::Succeeded),
        "Failed" => Ok(TaskStatus::Failed),
        "Retryable" => Ok(TaskStatus::Retryable),
        "Cancelled" => Ok(TaskStatus::Cancelled),
        _ => Err(rusqlite::Error::InvalidColumnType(
            4,
            "status".to_string(),
            rusqlite::types::Type::Text,
        )),
    }
}

#[derive(Debug, Clone)]
enum CredentialCodec {
    LegacyPlaintext,
    Aes256Gcm {
        passphrase: String,
    },
    MacOsKeychain {
        service_prefix: String,
        security_cli: String,
        fallback_passphrase: Option<String>,
    },
}

struct StoredCredential {
    key_material: String,
    storage_format: &'static str,
    storage_backend: &'static str,
    encrypted: bool,
    key_hint: Option<String>,
    reference: Option<Value>,
}

impl CredentialCodec {
    fn from_env() -> Self {
        let passphrase = std::env::var("POOL_CREDENTIAL_PASSPHRASE")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let credential_store = std::env::var("POOL_CREDENTIAL_STORE")
            .or_else(|_| std::env::var("POOL_CREDENTIAL_BACKEND"))
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();

        if matches!(credential_store.as_str(), "keychain" | "macos-keychain") {
            return Self::MacOsKeychain {
                service_prefix: std::env::var("POOL_KEYCHAIN_SERVICE_PREFIX")
                    .ok()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| "pool-runtime".to_string()),
                security_cli: std::env::var("POOL_SECURITY_CLI")
                    .ok()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| "security".to_string()),
                fallback_passphrase: passphrase,
            };
        }

        passphrase
            .map(|passphrase| Self::Aes256Gcm { passphrase })
            .unwrap_or(Self::LegacyPlaintext)
    }

    #[cfg(test)]
    fn passphrase(passphrase: impl Into<String>) -> Self {
        Self::Aes256Gcm {
            passphrase: passphrase.into(),
        }
    }

    #[cfg(test)]
    fn keychain_for_test(
        service_prefix: impl Into<String>,
        security_cli: impl Into<String>,
    ) -> Self {
        Self::MacOsKeychain {
            service_prefix: service_prefix.into(),
            security_cli: security_cli.into(),
            fallback_passphrase: None,
        }
    }

    fn seal(&self, provider: &str, service_type: &str, secret: &str) -> Result<StoredCredential> {
        match self {
            Self::LegacyPlaintext => Ok(StoredCredential {
                key_material: secret.to_string(),
                storage_format: "legacy-plaintext",
                storage_backend: "sqlite",
                encrypted: false,
                key_hint: key_hint_for_secret(secret),
                reference: None,
            }),
            Self::Aes256Gcm { passphrase } => {
                let nonce_bytes = uuid::Uuid::new_v4();
                let nonce = &nonce_bytes.as_bytes()[..12];
                let cipher = Aes256Gcm::new_from_slice(&derive_credential_key(
                    passphrase,
                    provider,
                    service_type,
                ))
                .context("create credential cipher")?;
                let ciphertext = cipher
                    .encrypt(Nonce::from_slice(nonce), secret.as_bytes())
                    .map_err(|_| anyhow::anyhow!("encrypt credential material"))?;
                Ok(StoredCredential {
                    key_material: format!(
                        "pool:v1:aes256gcm:{}:{}",
                        general_purpose::STANDARD_NO_PAD.encode(nonce),
                        general_purpose::STANDARD_NO_PAD.encode(ciphertext)
                    ),
                    storage_format: "pool:v1:aes256gcm",
                    storage_backend: "sqlite-encrypted",
                    encrypted: true,
                    key_hint: key_hint_for_secret(secret),
                    reference: None,
                })
            }
            Self::MacOsKeychain {
                service_prefix,
                security_cli,
                ..
            } => {
                let service = keychain_service_name(service_prefix, provider, service_type);
                let account = keychain_account_name(provider, service_type);
                write_keychain_secret(security_cli, &service, &account, secret)?;
                Ok(StoredCredential {
                    key_material: keychain_reference(&service, &account),
                    storage_format: "pool:v1:keychain",
                    storage_backend: "macos-keychain",
                    encrypted: true,
                    key_hint: key_hint_for_secret(secret),
                    reference: Some(serde_json_value!({
                        "service": service,
                        "account": account,
                    })),
                })
            }
        }
    }

    fn open(&self, stored: &str, provider: &str, service_type: &str) -> Result<String> {
        if stored.starts_with("pool:v1:keychain:") {
            let Self::MacOsKeychain { security_cli, .. } = self else {
                bail!(
                    "credential is stored in macOS Keychain; set POOL_CREDENTIAL_STORE=keychain before using {provider}/{service_type}"
                );
            };
            let (service, account) = parse_keychain_reference(stored)?;
            return read_keychain_secret(security_cli, &service, &account);
        }

        if !stored.starts_with("pool:v1:aes256gcm:") {
            if stored.starts_with("pool:v1:") {
                bail!("unsupported credential storage format");
            }
            return Ok(stored.to_string());
        }

        let passphrase = match self {
            Self::Aes256Gcm { passphrase } => passphrase,
            Self::MacOsKeychain {
                fallback_passphrase: Some(passphrase),
                ..
            } => passphrase,
            _ => {
                bail!(
                    "credential is encrypted; set POOL_CREDENTIAL_PASSPHRASE before using {provider}/{service_type}"
                );
            }
        };
        let mut parts = stored.split(':');
        let prefix = [
            parts.next().unwrap_or_default(),
            parts.next().unwrap_or_default(),
            parts.next().unwrap_or_default(),
        ]
        .join(":");
        if prefix != "pool:v1:aes256gcm" {
            bail!("unsupported credential storage format: {prefix}");
        }
        let nonce = parts.next().context("encrypted credential missing nonce")?;
        let ciphertext = parts
            .next()
            .context("encrypted credential missing ciphertext")?;
        if parts.next().is_some() {
            bail!("encrypted credential has unexpected trailing fields");
        }
        let nonce = general_purpose::STANDARD_NO_PAD
            .decode(nonce)
            .context("decode credential nonce")?;
        if nonce.len() != 12 {
            bail!("encrypted credential nonce must be 12 bytes");
        }
        let ciphertext = general_purpose::STANDARD_NO_PAD
            .decode(ciphertext)
            .context("decode encrypted credential")?;
        let cipher =
            Aes256Gcm::new_from_slice(&derive_credential_key(passphrase, provider, service_type))
                .context("create credential cipher")?;
        let plaintext = cipher
            .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
            .map_err(|_| anyhow::anyhow!("decrypt credential material"))?;
        String::from_utf8(plaintext).context("credential material is not valid utf-8")
    }
}

fn derive_credential_key(passphrase: &str, provider: &str, service_type: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"pool-runtime-credential-v1");
    hasher.update(passphrase.as_bytes());
    hasher.update(provider.as_bytes());
    hasher.update(service_type.as_bytes());
    hasher.finalize().into()
}

fn keychain_service_name(service_prefix: &str, provider: &str, service_type: &str) -> String {
    format!("{service_prefix}:{provider}:{service_type}")
}

fn keychain_account_name(provider: &str, service_type: &str) -> String {
    format!("{provider}/{service_type}")
}

fn keychain_reference(service: &str, account: &str) -> String {
    format!(
        "pool:v1:keychain:{}:{}",
        general_purpose::STANDARD_NO_PAD.encode(service),
        general_purpose::STANDARD_NO_PAD.encode(account)
    )
}

fn parse_keychain_reference(stored: &str) -> Result<(String, String)> {
    let mut parts = stored.split(':');
    let prefix = [
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
    ]
    .join(":");
    if prefix != "pool:v1:keychain" {
        bail!("unsupported keychain credential storage format: {prefix}");
    }
    let service = parts
        .next()
        .context("keychain credential missing service")?;
    let account = parts
        .next()
        .context("keychain credential missing account")?;
    if parts.next().is_some() {
        bail!("keychain credential has unexpected trailing fields");
    }
    let service = general_purpose::STANDARD_NO_PAD
        .decode(service)
        .context("decode keychain service")?;
    let account = general_purpose::STANDARD_NO_PAD
        .decode(account)
        .context("decode keychain account")?;
    Ok((
        String::from_utf8(service).context("keychain service is not valid utf-8")?,
        String::from_utf8(account).context("keychain account is not valid utf-8")?,
    ))
}

fn write_keychain_secret(
    security_cli: &str,
    service: &str,
    account: &str,
    secret: &str,
) -> Result<()> {
    let output = Command::new(security_cli)
        .arg("add-generic-password")
        .arg("-a")
        .arg(account)
        .arg("-s")
        .arg(service)
        .arg("-w")
        .arg(secret)
        .arg("-U")
        .output()
        .with_context(|| format!("run macOS security CLI at {security_cli}"))?;
    if !output.status.success() {
        bail!(
            "write credential to macOS Keychain failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn read_keychain_secret(security_cli: &str, service: &str, account: &str) -> Result<String> {
    let output = Command::new(security_cli)
        .arg("find-generic-password")
        .arg("-a")
        .arg(account)
        .arg("-s")
        .arg(service)
        .arg("-w")
        .output()
        .with_context(|| format!("run macOS security CLI at {security_cli}"))?;
    if !output.status.success() {
        bail!(
            "read credential from macOS Keychain failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let mut secret = String::from_utf8(output.stdout).context("keychain output is not utf-8")?;
    while secret.ends_with('\n') || secret.ends_with('\r') {
        secret.pop();
    }
    Ok(secret)
}

fn merge_credential_metadata(mut metadata: Value, stored: &StoredCredential) -> Result<Value> {
    if !metadata.is_object() {
        metadata = serde_json_value!({ "value": metadata });
    }
    let Some(map) = metadata.as_object_mut() else {
        bail!("api key metadata must be a JSON object");
    };
    let mut credential = serde_json_value!({
        "storage": stored.storage_format,
        "backend": stored.storage_backend,
        "encrypted": stored.encrypted,
        "key_hint": stored.key_hint,
    });
    if let Some(reference) = &stored.reference {
        credential["reference"] = reference.clone();
    }
    map.insert("credential".to_string(), credential);
    Ok(Value::Object(map.clone()))
}

fn key_hint_for_secret(secret: &str) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::build_default_content_burst_plan;

    #[test]
    fn migrates_and_persists_default_runtime_plan() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();

        let plan = build_default_content_burst_plan("demo", "Pool demo");
        let stats = repository.persist_plan(&plan).unwrap();

        assert_eq!(stats.projects, 1);
        assert_eq!(stats.shots, 1);
        assert_eq!(stats.workflows, 1);
        assert_eq!(stats.tasks, plan.workflow.nodes.len() as u64);
        assert_eq!(stats.events, 1);
    }

    #[test]
    fn rejects_unsafe_table_count_names() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();

        assert!(repository.table_count("tasks; DROP TABLE tasks").is_err());
    }

    #[test]
    fn inserts_provider_progress_event() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();

        repository
            .insert_event(&RuntimeEvent::new(
                "demo",
                RuntimeEventLevel::Info,
                "ComfyUI progress node 7: 3/10",
            ))
            .unwrap();

        assert_eq!(repository.stats().unwrap().events, 1);
    }

    #[test]
    fn indexes_local_outputs_as_asset_records() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let paths = vec![
            "worlds/demo/output/1-plate.png".to_string(),
            "worlds/demo/output/2-preview.mp4".to_string(),
        ];

        let assets = repository
            .index_local_outputs(
                "demo",
                Some("node-image"),
                Some("provider://comfyui"),
                &paths,
            )
            .unwrap();

        assert_eq!(assets.len(), 2);
        assert_eq!(assets[0].asset_type, "image");
        assert_eq!(assets[1].asset_type, "video");
        assert_eq!(repository.stats().unwrap().assets, 2);
    }

    #[test]
    fn updates_task_status() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let task = RuntimeTask::new("demo", "queued");
        let task_id = task.id.clone();

        repository.insert_task(&task).unwrap();
        repository
            .update_task_status(&task_id, TaskStatus::Succeeded)
            .unwrap();

        assert_eq!(repository.stats().unwrap().tasks, 1);
    }

    #[test]
    fn inserts_software_action_with_result() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let task = RuntimeTask::new("demo", "unreal create scene");
        let task_id = task.id.clone();
        repository.insert_task(&task).unwrap();
        let action = SoftwareControlAction {
            adapter_id: "unreal".to_string(),
            action_kind: crate::control::SoftwareActionKind::CreateScene,
            priority: crate::control::ControlPriority::ApiMcp,
            payload_json: serde_json::json!({"level":"demo"}),
            requires_confirmation: false,
        };
        let result = SoftwareActionResult {
            adapter_id: "unreal".to_string(),
            action_kind: crate::control::SoftwareActionKind::CreateScene,
            priority: crate::control::ControlPriority::ApiMcp,
            ok: true,
            message: "created".to_string(),
            artifacts: vec!["unreal://viewport".to_string()],
        };

        repository
            .insert_software_action("action-1", Some(&task_id), &action, Some(&result))
            .unwrap();

        assert_eq!(repository.table_count("software_actions").unwrap(), 1);
    }

    #[test]
    fn updates_software_action_verification() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let task = RuntimeTask::new("demo", "desktop control");
        let task_id = task.id.clone();
        repository.insert_task(&task).unwrap();
        let action = SoftwareControlAction {
            adapter_id: "touchdesigner".to_string(),
            action_kind: crate::control::SoftwareActionKind::RunViewport,
            priority: crate::control::ControlPriority::DesktopRecognition,
            payload_json: serde_json::json!({"target_window":"TouchDesigner"}),
            requires_confirmation: false,
        };
        let result = SoftwareActionResult {
            adapter_id: "touchdesigner".to_string(),
            action_kind: crate::control::SoftwareActionKind::RunViewport,
            priority: crate::control::ControlPriority::DesktopRecognition,
            ok: true,
            message: "staged".to_string(),
            artifacts: vec!["desktop-recognition://touchdesigner/1".to_string()],
        };

        repository
            .insert_software_action("action-1", Some(&task_id), &action, Some(&result))
            .unwrap();
        repository
            .update_software_action_verification(
                "action-1",
                serde_json::json!({"desktop_recognition_status":"succeeded"}),
            )
            .unwrap();

        let snapshot = repository.snapshot(None).unwrap();
        assert_eq!(
            snapshot.software_actions[0].verification.as_ref().unwrap()
                ["desktop_recognition_status"],
            "succeeded"
        );
    }

    #[test]
    fn inserts_agent_session_with_transcript_path() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let session = AgentSession::new("demo", vec!["mcp".to_string(), "git".to_string()])
            .with_token_budget(Some(5_000))
            .with_token_used(320)
            .with_transcript_path("worlds/demo/output/control/session.json");

        repository.insert_agent_session(&session).unwrap();
        let snapshot = repository.snapshot(None).unwrap();

        assert_eq!(repository.table_count("agent_sessions").unwrap(), 1);
        assert_eq!(snapshot.stats.agent_token_used, 320);
        assert_eq!(snapshot.stats.agent_token_budget, 5_000);
        assert_eq!(snapshot.stats.token_total, 320);
        assert_eq!(snapshot.stats.budget_remaining, Some(4_680));
    }

    #[test]
    fn upserts_api_key_without_exposing_secret_in_snapshot() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();

        let key = repository
            .upsert_api_key(
                "openai-image-2",
                "provider",
                "sk-test-secret",
                serde_json::json!({"env":"OPENAI_API_KEY"}),
            )
            .unwrap();
        let snapshot = repository.snapshot(None).unwrap();

        assert!(key.configured);
        assert_eq!(key.key_hint.as_deref(), Some("...cret"));
        assert_eq!(
            repository
                .api_key_secret("openai-image-2", "provider")
                .unwrap()
                .as_deref(),
            Some("sk-test-secret")
        );
        assert_eq!(snapshot.stats.api_keys, 1);
        assert_eq!(snapshot.api_keys[0].key_hint.as_deref(), Some("...cret"));
        assert!(!serde_json::to_string(&snapshot)
            .unwrap()
            .contains("sk-test-secret"));
    }

    #[test]
    fn upserts_api_key_with_encrypted_local_storage() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let codec = CredentialCodec::passphrase("local-test-passphrase");

        let key = repository
            .upsert_api_key_with_codec(
                "suno",
                "provider",
                "suno-test-secret",
                serde_json::json!({"env":"POOL_SUNO_API_KEY"}),
                &codec,
            )
            .unwrap();
        let raw: String = repository
            .connection
            .query_row(
                "SELECT encrypted_key FROM api_keys WHERE provider = 'suno'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let snapshot = repository.snapshot(None).unwrap();

        assert!(key.configured);
        assert!(raw.starts_with("pool:v1:aes256gcm:"));
        assert!(!raw.contains("suno-test-secret"));
        assert_eq!(
            repository
                .api_key_secret_with_codec("suno", "provider", &codec)
                .unwrap()
                .as_deref(),
            Some("suno-test-secret")
        );
        assert_eq!(snapshot.api_keys[0].key_hint.as_deref(), Some("...cret"));
        assert_eq!(
            snapshot.api_keys[0].metadata["credential"]["storage"],
            "pool:v1:aes256gcm"
        );
        assert!(!serde_json::to_string(&snapshot)
            .unwrap()
            .contains("suno-test-secret"));
    }

    #[cfg(unix)]
    #[test]
    fn upserts_api_key_with_macos_keychain_reference() {
        use std::os::unix::fs::PermissionsExt;

        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let temp_dir =
            std::env::temp_dir().join(format!("pool-keychain-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let script_path = temp_dir.join("security");
        let store_path = temp_dir.join("store.txt");
        std::fs::write(
            &script_path,
            format!(
                r#"#!/bin/sh
store="{}"
mode="$1"
shift
account=""
service=""
password=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -a) shift; account="$1" ;;
    -s) shift; service="$1" ;;
    -w)
      if [ "$mode" = "add-generic-password" ]; then
        shift
        password="$1"
      fi
      ;;
  esac
  shift
done
if [ "$mode" = "add-generic-password" ]; then
  printf '%s\n%s\n%s\n' "$service" "$account" "$password" > "$store"
  exit 0
fi
if [ "$mode" = "find-generic-password" ]; then
  tail -n 1 "$store"
  exit 0
fi
exit 2
"#,
                store_path.display()
            ),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&script_path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script_path, permissions).unwrap();
        let codec = CredentialCodec::keychain_for_test(
            "pool-test",
            script_path.to_string_lossy().to_string(),
        );

        let key = repository
            .upsert_api_key_with_codec(
                "suno",
                "provider",
                "suno-keychain-secret",
                serde_json::json!({"env":"POOL_SUNO_API_KEY"}),
                &codec,
            )
            .unwrap();
        let raw: String = repository
            .connection
            .query_row(
                "SELECT encrypted_key FROM api_keys WHERE provider = 'suno'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let snapshot = repository.snapshot(None).unwrap();

        assert!(key.configured);
        assert!(raw.starts_with("pool:v1:keychain:"));
        assert!(!raw.contains("suno-keychain-secret"));
        assert_eq!(
            repository
                .api_key_secret_with_codec("suno", "provider", &codec)
                .unwrap()
                .as_deref(),
            Some("suno-keychain-secret")
        );
        assert_eq!(snapshot.api_keys[0].key_hint.as_deref(), Some("...cret"));
        assert_eq!(
            snapshot.api_keys[0].metadata["credential"]["storage"],
            "pool:v1:keychain"
        );
        assert_eq!(
            snapshot.api_keys[0].metadata["credential"]["backend"],
            "macos-keychain"
        );
        assert_eq!(
            snapshot.api_keys[0].metadata["credential"]["reference"]["service"],
            "pool-test:suno:provider"
        );
        assert!(!serde_json::to_string(&snapshot)
            .unwrap()
            .contains("suno-keychain-secret"));
        std::fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn builds_runtime_snapshot_for_default_plan() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool demo");
        repository.persist_plan(&plan).unwrap();

        let snapshot = repository.snapshot(Some("demo")).unwrap();

        assert_eq!(snapshot.version, 1);
        assert_eq!(snapshot.stats.projects, 1);
        assert_eq!(snapshot.stats.workflows, 1);
        assert_eq!(snapshot.stats.tasks, plan.workflow.nodes.len());
        assert_eq!(snapshot.stats.waiting_approval, 1);
        assert_eq!(snapshot.stats.task_estimated_tokens, 9_000);
        assert_eq!(snapshot.stats.waiting_approval_estimated_tokens, 9_000);
        assert_eq!(snapshot.stats.token_total, 9_000);
        assert_eq!(snapshot.node_states.len(), plan.workflow.nodes.len());
        assert_eq!(snapshot.project_filter.as_deref(), Some("demo"));
    }

    #[test]
    fn approve_task_releases_waiting_approval_gate() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool demo");
        repository.persist_plan(&plan).unwrap();
        let task_id = repository
            .snapshot(Some("demo"))
            .unwrap()
            .tasks
            .into_iter()
            .find(|task| task.status == "WaitingApproval")
            .unwrap()
            .id;

        let approved = repository.approve_task(&task_id).unwrap();
        let snapshot = repository.snapshot(Some("demo")).unwrap();

        assert_eq!(approved.status, "Ready");
        assert_eq!(snapshot.stats.waiting_approval, 0);
        assert!(snapshot
            .events
            .iter()
            .any(|event| event.message.contains("approved task")));
    }

    #[test]
    fn approve_task_rejects_non_waiting_task() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let plan = build_default_content_burst_plan("demo", "Pool demo");
        repository.persist_plan(&plan).unwrap();
        let task_id = repository
            .snapshot(Some("demo"))
            .unwrap()
            .tasks
            .into_iter()
            .find(|task| task.status == "Ready")
            .unwrap()
            .id;

        let error = repository.approve_task(&task_id).unwrap_err();

        assert!(error.to_string().contains("not waiting for approval"));
    }

    #[test]
    fn snapshot_includes_provider_request_ledger() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let mut task = RuntimeTask::new("demo", "Provider ledger task");
        task.provider_id = Some("worldlabs-marble".to_string());
        repository.insert_task(&task).unwrap();
        let record = repository
            .insert_provider_request(
                &task.id,
                "worldlabs-marble",
                &serde_json_value!({
                    "provider_id": "worldlabs-marble",
                    "provider_request": {
                        "project_slug": "demo",
                        "prompt": "make world",
                        "input_paths": [],
                        "output_dir": "worlds/demo/output",
                        "require_approval": true
                    }
                }),
                Some("worlds/demo/output/.1-world-request.json"),
            )
            .unwrap();
        repository
            .update_provider_request_response(
                &record.id,
                &serde_json_value!({"status":"WaitingApproval"}),
                None,
            )
            .unwrap();

        let snapshot = repository.snapshot(Some("demo")).unwrap();

        assert_eq!(snapshot.stats.provider_requests, 1);
        assert_eq!(snapshot.provider_requests.len(), 1);
        assert_eq!(
            snapshot.provider_requests[0].project_slug.as_deref(),
            Some("demo")
        );
        assert_eq!(
            snapshot.provider_requests[0].provider_id,
            "worldlabs-marble"
        );
        assert_eq!(
            snapshot.provider_requests[0].response.as_ref().unwrap()["status"],
            "WaitingApproval"
        );
    }
}
