use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    Queued,
    Ready,
    Running,
    WaitingApproval,
    Succeeded,
    Failed,
    Retryable,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AssetStatus {
    Indexed,
    Local,
    Missing,
    RemoteOnly,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProviderKind {
    AiImage,
    AiVideo,
    ThreeDgs,
    Audio,
    Software,
    Agent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEnvelope {
    pub slug: String,
    pub root: String,
    pub project_json: String,
    pub workflow_json: String,
    pub scene_json: String,
    pub source_dir: String,
    pub output_dir: String,
    pub requests_dir: String,
    pub control_dir: String,
}

impl ProjectEnvelope {
    pub fn for_slug(slug: impl Into<String>) -> Self {
        let slug = slug.into();
        let root = format!("worlds/{slug}");
        Self {
            project_json: format!("{root}/project.json"),
            workflow_json: format!("{root}/workflow.json"),
            scene_json: format!("{root}/scene.json"),
            source_dir: format!("{root}/source"),
            output_dir: format!("{root}/output"),
            requests_dir: format!("{root}/output/requests"),
            control_dir: format!("{root}/output/control"),
            root,
            slug,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeTask {
    pub id: String,
    pub project_slug: String,
    pub node_id: Option<String>,
    pub title: String,
    pub status: TaskStatus,
    pub provider_id: Option<String>,
    pub cost_estimate_tokens: u64,
    pub requires_approval: bool,
    pub request_metadata_path: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl RuntimeTask {
    pub fn new(project_slug: impl Into<String>, title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            project_slug: project_slug.into(),
            node_id: None,
            title: title.into(),
            status: TaskStatus::Queued,
            provider_id: None,
            cost_estimate_tokens: 0,
            requires_approval: false,
            request_metadata_path: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_approval_gate(mut self, required: bool) -> Self {
        self.requires_approval = required;
        if required {
            self.status = TaskStatus::WaitingApproval;
        }
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetRecord {
    pub id: String,
    pub project_slug: String,
    pub name: String,
    pub asset_type: String,
    pub local_path: String,
    pub source_node_id: Option<String>,
    pub provider_url: Option<String>,
    pub status: AssetStatus,
    pub created_at: DateTime<Utc>,
}

impl AssetRecord {
    pub fn local(
        project_slug: impl Into<String>,
        name: impl Into<String>,
        local_path: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            project_slug: project_slug.into(),
            name: name.into(),
            asset_type: "unknown".to_string(),
            local_path: local_path.into(),
            source_node_id: None,
            provider_url: None,
            status: AssetStatus::Local,
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub display_name: String,
    pub kind: ProviderKind,
    pub endpoint: String,
    pub auth_env_key: Option<String>,
    pub output_contract: String,
    pub high_cost: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareAdapterConfig {
    pub id: String,
    pub display_name: String,
    pub control_modes: Vec<String>,
    pub priority: u8,
    pub desktop_fallback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalGate {
    pub id: String,
    pub task_id: String,
    pub reason: String,
    pub approved: bool,
    pub created_at: DateTime<Utc>,
    pub approved_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: String,
    pub project_slug: String,
    pub tools: Vec<String>,
    pub token_budget: Option<u64>,
    pub token_used: u64,
    pub transcript_path: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AgentSession {
    pub fn new(project_slug: impl Into<String>, tools: Vec<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            project_slug: project_slug.into(),
            tools,
            token_budget: None,
            token_used: 0,
            transcript_path: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_token_budget(mut self, token_budget: Option<u64>) -> Self {
        self.token_budget = token_budget;
        self
    }

    pub fn with_token_used(mut self, token_used: u64) -> Self {
        self.token_used = token_used;
        self
    }

    pub fn with_transcript_path(mut self, transcript_path: impl Into<String>) -> Self {
        self.transcript_path = Some(transcript_path.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RuntimeEventLevel {
    Info,
    Ok,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeEvent {
    pub id: String,
    pub project_slug: String,
    pub level: RuntimeEventLevel,
    pub message: String,
    pub created_at: DateTime<Utc>,
}

impl RuntimeEvent {
    pub fn new(
        project_slug: impl Into<String>,
        level: RuntimeEventLevel,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            project_slug: project_slug.into(),
            level,
            message: message.into(),
            created_at: Utc::now(),
        }
    }
}
