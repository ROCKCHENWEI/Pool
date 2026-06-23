use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProjectStatus {
    Draft,
    Active,
    Paused,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ShotStatus {
    Draft,
    Ready,
    Running,
    Blocked,
    Approved,
    Delivered,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SegmentKind {
    VideoShot,
    GameLevelSection,
    InteractiveCue,
    AudioCue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OutputTarget {
    Video,
    Game,
    InteractiveArt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub status: ProjectStatus,
    pub output_targets: Vec<OutputTarget>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Project {
    pub fn new(slug: impl Into<String>, title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            slug: slug.into(),
            title: title.into(),
            status: ProjectStatus::Draft,
            output_targets: vec![
                OutputTarget::Video,
                OutputTarget::Game,
                OutputTarget::InteractiveArt,
            ],
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shot {
    pub id: String,
    pub project_slug: String,
    pub title: String,
    pub status: ShotStatus,
    pub timeline_start_ms: u64,
    pub duration_ms: u64,
    pub segments: Vec<Segment>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Shot {
    pub fn new(
        project_slug: impl Into<String>,
        title: impl Into<String>,
        timeline_start_ms: u64,
        duration_ms: u64,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            project_slug: project_slug.into(),
            title: title.into(),
            status: ShotStatus::Draft,
            timeline_start_ms,
            duration_ms,
            segments: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn push_segment(&mut self, segment: Segment) {
        self.segments.push(segment);
        self.updated_at = Utc::now();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub id: String,
    pub title: String,
    pub kind: SegmentKind,
    pub output_target: OutputTarget,
    pub workflow_id: Option<String>,
}

impl Segment {
    pub fn new(title: impl Into<String>, kind: SegmentKind, output_target: OutputTarget) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            title: title.into(),
            kind,
            output_target,
            workflow_id: None,
        }
    }
}
