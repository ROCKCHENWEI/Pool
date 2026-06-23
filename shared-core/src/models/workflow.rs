use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeType {
    Input,
    Prompt,
    Storyboard,
    Agent,
    AgentCli,
    Hermes,
    AiImage,
    AiVideo,
    Audio,
    ComfyUi,
    ThreeDgs,
    AssetPackage,
    SoftwareControl,
    Unreal,
    Blender,
    Resolve,
    Unity,
    TouchDesigner,
    MadMapper,
    Nuke,
    MotionCaptureDb,
    Suno,
    ApprovalGate,
    VideoOutput,
    GameOutput,
    InteractiveOutput,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeStatus {
    Idle,
    Ready,
    Running,
    WaitingApproval,
    Succeeded,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConnectionKind {
    AssetFlow,
    ControlFlow,
    AgentInstruction,
    FeedbackLoop,
    Approval,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowNode {
    pub id: String,
    pub title: String,
    pub node_type: NodeType,
    pub status: NodeStatus,
    pub provider_id: Option<String>,
    pub software_adapter_id: Option<String>,
    pub requires_approval: bool,
    pub cost_estimate_tokens: u64,
    pub parameters: Value,
    pub position: Option<NodePosition>,
}

impl WorkflowNode {
    pub fn new(title: impl Into<String>, node_type: NodeType) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            title: title.into(),
            node_type,
            status: NodeStatus::Idle,
            provider_id: None,
            software_adapter_id: None,
            requires_approval: false,
            cost_estimate_tokens: 0,
            parameters: Value::Object(Default::default()),
            position: None,
        }
    }

    pub fn with_provider(mut self, provider_id: impl Into<String>) -> Self {
        self.provider_id = Some(provider_id.into());
        self
    }

    pub fn with_software_adapter(mut self, software_adapter_id: impl Into<String>) -> Self {
        self.software_adapter_id = Some(software_adapter_id.into());
        self
    }

    pub fn with_high_cost_approval(mut self, cost_estimate_tokens: u64) -> Self {
        self.requires_approval = true;
        self.cost_estimate_tokens = cost_estimate_tokens;
        self.status = NodeStatus::WaitingApproval;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePosition {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowConnection {
    pub id: String,
    pub from_node_id: String,
    pub to_node_id: String,
    pub kind: ConnectionKind,
    pub label: String,
}

impl WorkflowConnection {
    pub fn new(
        from_node_id: impl Into<String>,
        to_node_id: impl Into<String>,
        kind: ConnectionKind,
        label: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            from_node_id: from_node_id.into(),
            to_node_id: to_node_id.into(),
            kind,
            label: label.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub id: String,
    pub project_slug: String,
    pub title: String,
    pub nodes: BTreeMap<String, WorkflowNode>,
    pub connections: Vec<WorkflowConnection>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Workflow {
    pub fn new(project_slug: impl Into<String>, title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            project_slug: project_slug.into(),
            title: title.into(),
            nodes: BTreeMap::new(),
            connections: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn add_node(&mut self, node: WorkflowNode) -> String {
        let id = node.id.clone();
        self.nodes.insert(id.clone(), node);
        self.updated_at = Utc::now();
        id
    }

    pub fn connect(
        &mut self,
        from_node_id: impl Into<String>,
        to_node_id: impl Into<String>,
        kind: ConnectionKind,
        label: impl Into<String>,
    ) -> String {
        let connection = WorkflowConnection::new(from_node_id, to_node_id, kind, label);
        let id = connection.id.clone();
        self.connections.push(connection);
        self.updated_at = Utc::now();
        id
    }
}
