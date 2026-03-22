use pool_core::comfyui::ComfyUIClient;
use pool_core::models::{ComfyUIConfig, ComfyUITemplateLibrary, ComfyUIInput, ComfyUIInputType};

#[tokio::test]
async fn test_comfyui_client_creation() {
    let client = ComfyUIClient::new("http://127.0.0.1:8188");
    assert_eq!(client.base_url(), "http://127.0.0.1:8188");
}

#[tokio::test]
async fn test_comfyui_client_trailing_slash() {
    let client = ComfyUIClient::new("http://127.0.0.1:8188/");
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

#[test]
fn test_comfyui_config_default() {
    let config = ComfyUIConfig::default();
    assert_eq!(config.server_url, "http://127.0.0.1:8188");
    assert_eq!(config.timeout_secs, 30);
    assert!(config.auto_reconnect);
    assert_eq!(config.max_retries, 3);
}

#[test]
fn test_comfyui_config_custom_url() {
    let config = ComfyUIConfig::new("http://192.168.1.100:8188".to_string());
    assert_eq!(config.server_url, "http://192.168.1.100:8188");
    assert_eq!(config.websocket_url, "ws://192.168.1.100:8188/ws");
}

#[test]
fn test_comfyui_config_validation() {
    let valid_config = ComfyUIConfig::default();
    assert!(valid_config.validate().is_ok());

    let invalid_config = ComfyUIConfig {
        server_url: "".to_string(),
        ..Default::default()
    };
    assert!(invalid_config.validate().is_err());

    let invalid_protocol = ComfyUIConfig {
        server_url: "ftp://invalid.com".to_string(),
        ..Default::default()
    };
    assert!(invalid_protocol.validate().is_err());
}

#[test]
fn test_template_library() {
    let templates = ComfyUITemplateLibrary::get_templates();
    assert!(!templates.is_empty());

    // Check basic text-to-image template
    let t2i = templates.iter().find(|t| t.name == "Basic Text-to-Image");
    assert!(t2i.is_some());

    let template = t2i.unwrap();
    assert_eq!(template.category, "Text-to-Image");
    assert!(!template.required_inputs.is_empty());
}

#[test]
fn test_comfyui_input_types() {
    let string_input = ComfyUIInput {
        name: "prompt".to_string(),
        input_type: ComfyUIInputType::String,
        default_value: Some("test".to_string()),
        required: true,
        description: "Test input".to_string(),
    };

    assert!(string_input.required);
    assert_eq!(string_input.name, "prompt");
}

#[test]
fn test_comfyui_config_websocket_url_generation() {
    let config_http = ComfyUIConfig::new("http://localhost:8188".to_string());
    assert_eq!(config_http.websocket_url, "ws://localhost:8188/ws");

    let config_https = ComfyUIConfig::new("https://example.com:8188".to_string());
    assert_eq!(config_https.websocket_url, "wss://example.com:8188/ws");
}
