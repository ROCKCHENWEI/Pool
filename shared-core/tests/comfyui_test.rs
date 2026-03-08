use pool_core::comfyui::ComfyUIClient;

#[tokio::test]
async fn test_comfyui_client_creation() {
    let client = ComfyUIClient::new("http://127.0.0.1:8188");
    assert_eq!(client.base_url(), "http://127.0.0.1:8188");
}

#[tokio::test]
async fn test_workflow_translation() {
    use pool_core::comfyui::WorkflowTranslator;
    use pool_core::models::{Node, NodeType, Workflow};
    use std::collections::HashMap;

    let mut workflow = Workflow::new("test".to_string(), "shot_1".to_string());

    workflow.nodes.push(Node {
        id: "text_1".to_string(),
        node_type: NodeType::TextPrompt,
        position: (0.0, 0.0),
        params: {
            let mut p = HashMap::new();
            p.insert(
                "prompt".to_string(),
                pool_core::models::NodeParam::String("a cat".to_string()),
            );
            p
        },
    });

    let mut translator = WorkflowTranslator::new();
    let comfy_workflow = translator.translate(&workflow);

    assert!(comfy_workflow.contains_key("0"));
}
