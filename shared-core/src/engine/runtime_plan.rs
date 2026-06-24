use crate::control::default_software_adapters;
use crate::models::{
    ConnectionKind, NodePosition, NodeType, OutputTarget, Project, ProjectEnvelope, ProviderConfig,
    Segment, SegmentKind, Shot, SoftwareAdapterConfig, Workflow, WorkflowNode,
};
use crate::providers::default_provider_configs;

#[derive(Debug, Clone)]
pub struct PoolRuntimePlan {
    pub project: Project,
    pub envelope: ProjectEnvelope,
    pub shots: Vec<Shot>,
    pub workflow: Workflow,
    pub providers: Vec<ProviderConfig>,
    pub software_adapters: Vec<SoftwareAdapterConfig>,
}

pub fn build_default_content_burst_plan(slug: &str, title: &str) -> PoolRuntimePlan {
    let project = Project::new(slug, title);
    let envelope = ProjectEnvelope::for_slug(slug);
    let providers = default_provider_configs();
    let software_adapters = default_software_adapters();
    let mut workflow = Workflow::new(slug, "creative input to multi-output runtime");

    let input = workflow.add_node(positioned(
        WorkflowNode::new("起始输入", NodeType::Input),
        0.0,
        120.0,
    ));
    let agent = workflow.add_node(positioned(
        WorkflowNode::new("Agent 创意分析", NodeType::Agent).with_provider("hermes"),
        240.0,
        120.0,
    ));
    let image = workflow.add_node(positioned(
        WorkflowNode::new("AI 图片生成", NodeType::AiImage).with_provider("comfyui"),
        480.0,
        40.0,
    ));
    let three_dgs = workflow.add_node(positioned(
        WorkflowNode::new("2D/3DGS 转换", NodeType::ThreeDgs)
            .with_provider("worldlabs-marble")
            .with_high_cost_approval(9_000),
        720.0,
        120.0,
    ));
    let asset_package = workflow.add_node(positioned(
        WorkflowNode::new("本地资产包", NodeType::AssetPackage),
        960.0,
        120.0,
    ));
    let unreal = workflow.add_node(positioned(
        WorkflowNode::new("Unreal 拼装", NodeType::Unreal).with_software_adapter("unreal"),
        1_200.0,
        120.0,
    ));
    let video = workflow.add_node(positioned(
        WorkflowNode::new("视频输出", NodeType::VideoOutput).with_software_adapter("resolve"),
        1_480.0,
        20.0,
    ));
    let game = workflow.add_node(positioned(
        WorkflowNode::new("游戏输出", NodeType::GameOutput).with_software_adapter("unreal"),
        1_480.0,
        120.0,
    ));
    let interactive = workflow.add_node(positioned(
        WorkflowNode::new("交互艺术输出", NodeType::InteractiveOutput)
            .with_software_adapter("touchdesigner"),
        1_480.0,
        220.0,
    ));

    workflow.connect(
        &input,
        &agent,
        ConnectionKind::AgentInstruction,
        "brief + reference",
    );
    workflow.connect(&agent, &image, ConnectionKind::ControlFlow, "prompt plan");
    workflow.connect(
        &image,
        &three_dgs,
        ConnectionKind::AssetFlow,
        "generated plates",
    );
    workflow.connect(
        &three_dgs,
        &asset_package,
        ConnectionKind::Approval,
        "cost gate + localize",
    );
    workflow.connect(
        &asset_package,
        &unreal,
        ConnectionKind::AssetFlow,
        "glb/spz/scene import",
    );
    workflow.connect(
        &unreal,
        &video,
        ConnectionKind::ControlFlow,
        "camera timeline",
    );
    workflow.connect(
        &unreal,
        &game,
        ConnectionKind::ControlFlow,
        "level viewport",
    );
    workflow.connect(
        &unreal,
        &interactive,
        ConnectionKind::ControlFlow,
        "realtime cue graph",
    );

    let mut shot = Shot::new(slug, "首个内容爆发片段", 0, 12_000);
    let mut video_segment = Segment::new("镜头时间线", SegmentKind::VideoShot, OutputTarget::Video);
    video_segment.workflow_id = Some(workflow.id.clone());
    shot.push_segment(video_segment);

    let mut game_segment = Segment::new(
        "运行关卡片段",
        SegmentKind::GameLevelSection,
        OutputTarget::Game,
    );
    game_segment.workflow_id = Some(workflow.id.clone());
    shot.push_segment(game_segment);

    let mut cue_segment = Segment::new(
        "声光电 cue",
        SegmentKind::InteractiveCue,
        OutputTarget::InteractiveArt,
    );
    cue_segment.workflow_id = Some(workflow.id.clone());
    shot.push_segment(cue_segment);

    PoolRuntimePlan {
        project,
        envelope,
        shots: vec![shot],
        workflow,
        providers,
        software_adapters,
    }
}

fn positioned(mut node: WorkflowNode, x: f32, y: f32) -> WorkflowNode {
    node.position = Some(NodePosition { x, y });
    node
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::NodeEngine;
    use crate::models::NodeStatus;

    #[test]
    fn default_plan_is_acyclic_and_contains_required_outputs() {
        let plan = build_default_content_burst_plan("demo", "Pool demo");

        NodeEngine::validate_acyclic(&plan.workflow).unwrap();
        assert_eq!(plan.envelope.root, "worlds/demo");
        assert_eq!(plan.shots[0].segments.len(), 3);
        assert!(plan
            .providers
            .iter()
            .any(|provider| provider.id == "comfyui"));
        assert!(plan
            .software_adapters
            .iter()
            .any(|adapter| adapter.id == "unreal"));

        let waiting_approval = plan
            .workflow
            .nodes
            .values()
            .find(|node| node.node_type == NodeType::ThreeDgs)
            .unwrap();
        assert_eq!(waiting_approval.status, NodeStatus::WaitingApproval);
        assert!(waiting_approval.requires_approval);
    }
}
