//! ComfyUI Configuration Module
//!
//! Provides configuration and connection management for ComfyUI server integration.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// ComfyUI server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComfyUIConfig {
    /// Server base URL (e.g., "http://127.0.0.1:8188")
    pub server_url: String,
    /// WebSocket URL (e.g., "ws://127.0.0.1:8188/ws")
    pub websocket_url: String,
    /// Connection timeout in seconds
    pub timeout_secs: u64,
    /// Enable automatic reconnection
    pub auto_reconnect: bool,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Client ID for WebSocket connections
    pub client_id: String,
}

impl Default for ComfyUIConfig {
    fn default() -> Self {
        Self {
            server_url: "http://127.0.0.1:8188".to_string(),
            websocket_url: "ws://127.0.0.1:8188/ws".to_string(),
            timeout_secs: 30,
            auto_reconnect: true,
            max_retries: 3,
            client_id: uuid::Uuid::new_v4().to_string(),
        }
    }
}

impl ComfyUIConfig {
    /// Create a new configuration with custom server URL
    pub fn new(server_url: String) -> Self {
        let websocket_url = server_url
            .replace("http://", "ws://")
            .replace("https://", "wss://")
            + "/ws";
        Self {
            server_url,
            websocket_url,
            ..Default::default()
        }
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.server_url.is_empty() {
            return Err("Server URL cannot be empty".to_string());
        }
        if !self.server_url.starts_with("http://") && !self.server_url.starts_with("https://") {
            return Err("Server URL must start with http:// or https://".to_string());
        }
        Ok(())
    }
}

/// ComfyUI workflow template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComfyUIWorkflowTemplate {
    /// Template name
    pub name: String,
    /// Template description
    pub description: String,
    /// Template category (e.g., "Text-to-Image", "Image-to-Image")
    pub category: String,
    /// Required inputs
    pub required_inputs: Vec<ComfyUIInput>,
    /// Workflow JSON structure
    pub workflow_json: HashMap<String, serde_json::Value>,
}

/// ComfyUI input parameter definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComfyUIInput {
    /// Input name
    pub name: String,
    /// Input type
    pub input_type: ComfyUIInputType,
    /// Default value
    pub default_value: Option<String>,
    /// Is required
    pub required: bool,
    /// Description
    pub description: String,
}

/// ComfyUI input types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComfyUIInputType {
    String,
    Integer,
    Float,
    Boolean,
    Image,
    Model,
}

/// Predefined ComfyUI templates
pub struct ComfyUITemplateLibrary;

impl ComfyUITemplateLibrary {
    /// Get all available templates
    pub fn get_templates() -> Vec<ComfyUIWorkflowTemplate> {
        vec![
            Self::basic_text_to_image(),
            Self::basic_image_to_image(),
            Self::controlnet_workflow(),
            Self::upscale_enhance(),
            Self::style_transfer(),
            Self::inpainting(),
            Self::face_restore(),
            Self::video_generation(),
            Self::batch_processing(),
        ]
    }

    /// Basic text-to-image template
    fn basic_text_to_image() -> ComfyUIWorkflowTemplate {
        let mut workflow = HashMap::new();
        workflow.insert("3".to_string(), serde_json::json!({
            "class_type": "KSampler",
            "inputs": {
                "seed": 123456789,
                "steps": 20,
                "cfg": 7.0,
                "sampler_name": "euler",
                "scheduler": "normal",
                "denoise": 1.0,
                "model": ["4", 0],
                "positive": ["6", 0],
                "negative": ["7", 0],
                "latent_image": ["5", 0]
            }
        }));
        workflow.insert("4".to_string(), serde_json::json!({
            "class_type": "CheckpointLoaderSimple",
            "inputs": { "ckpt_name": "v1-5-pruned.safetensors" }
        }));
        workflow.insert("5".to_string(), serde_json::json!({
            "class_type": "EmptyLatentImage",
            "inputs": { "width": 512, "height": 512, "batch_size": 1 }
        }));
        workflow.insert("6".to_string(), serde_json::json!({
            "class_type": "CLIPTextEncode",
            "inputs": { "text": "", "clip": ["4", 1] }
        }));
        workflow.insert("7".to_string(), serde_json::json!({
            "class_type": "CLIPTextEncode",
            "inputs": { "text": "", "clip": ["4", 1] }
        }));
        workflow.insert("8".to_string(), serde_json::json!({
            "class_type": "VAEDecode",
            "inputs": { "samples": ["3", 0], "vae": ["4", 2] }
        }));
        workflow.insert("9".to_string(), serde_json::json!({
            "class_type": "SaveImage",
            "inputs": { "filename_prefix": "Pool_", "images": ["8", 0] }
        }));

        ComfyUIWorkflowTemplate {
            name: "Basic Text-to-Image".to_string(),
            description: "Generate images from text prompts using Stable Diffusion".to_string(),
            category: "Text-to-Image".to_string(),
            required_inputs: vec![
                ComfyUIInput {
                    name: "positive_prompt".to_string(),
                    input_type: ComfyUIInputType::String,
                    default_value: Some("".to_string()),
                    required: true,
                    description: "Positive prompt for image generation".to_string(),
                },
                ComfyUIInput {
                    name: "negative_prompt".to_string(),
                    input_type: ComfyUIInputType::String,
                    default_value: Some("".to_string()),
                    required: false,
                    description: "Negative prompt to avoid certain features".to_string(),
                },
            ],
            workflow_json: workflow,
        }
    }

    /// Basic image-to-image template
    fn basic_image_to_image() -> ComfyUIWorkflowTemplate {
        ComfyUIWorkflowTemplate {
            name: "Basic Image-to-Image".to_string(),
            description: "Transform existing images based on text prompts".to_string(),
            category: "Image-to-Image".to_string(),
            required_inputs: vec![
                ComfyUIInput {
                    name: "image".to_string(),
                    input_type: ComfyUIInputType::Image,
                    default_value: None,
                    required: true,
                    description: "Input image to transform".to_string(),
                },
                ComfyUIInput {
                    name: "prompt".to_string(),
                    input_type: ComfyUIInputType::String,
                    default_value: Some("".to_string()),
                    required: true,
                    description: "Transformation prompt".to_string(),
                },
            ],
            workflow_json: HashMap::new(), // Simplified for now
        }
    }

    /// ControlNet workflow template
    fn controlnet_workflow() -> ComfyUIWorkflowTemplate {
        ComfyUIWorkflowTemplate {
            name: "ControlNet Workflow".to_string(),
            description: "Use ControlNet for precise image control".to_string(),
            category: "Advanced".to_string(),
            required_inputs: vec![
                ComfyUIInput {
                    name: "control_image".to_string(),
                    input_type: ComfyUIInputType::Image,
                    default_value: None,
                    required: true,
                    description: "Control image for guidance".to_string(),
                },
                ComfyUIInput {
                    name: "prompt".to_string(),
                    input_type: ComfyUIInputType::String,
                    default_value: Some("".to_string()),
                    required: true,
                    description: "Text prompt".to_string(),
                },
            ],
            workflow_json: HashMap::new(), // Simplified for now
        }
    }

    /// Upscale and enhance template
    fn upscale_enhance() -> ComfyUIWorkflowTemplate {
        ComfyUIWorkflowTemplate {
            name: "Upscale & Enhance".to_string(),
            description: "Upscale images with AI enhancement and detail recovery".to_string(),
            category: "Image Enhancement".to_string(),
            required_inputs: vec![
                ComfyUIInput {
                    name: "image".to_string(),
                    input_type: ComfyUIInputType::Image,
                    default_value: None,
                    required: true,
                    description: "Image to upscale".to_string(),
                },
                ComfyUIInput {
                    name: "scale_factor".to_string(),
                    input_type: ComfyUIInputType::Float,
                    default_value: Some("2.0".to_string()),
                    required: false,
                    description: "Upscaling factor (1.5-4x)".to_string(),
                },
                ComfyUIInput {
                    name: "denoise_strength".to_string(),
                    input_type: ComfyUIInputType::Float,
                    default_value: Some("0.35".to_string()),
                    required: false,
                    description: "Denoising strength for enhancement (0.0-1.0)".to_string(),
                },
            ],
            workflow_json: HashMap::new(),
        }
    }

    /// Style transfer template
    fn style_transfer() -> ComfyUIWorkflowTemplate {
        ComfyUIWorkflowTemplate {
            name: "Style Transfer".to_string(),
            description: "Transfer artistic style from one image to another".to_string(),
            category: "Artistic".to_string(),
            required_inputs: vec![
                ComfyUIInput {
                    name: "content_image".to_string(),
                    input_type: ComfyUIInputType::Image,
                    default_value: None,
                    required: true,
                    description: "Content image to apply style to".to_string(),
                },
                ComfyUIInput {
                    name: "style_prompt".to_string(),
                    input_type: ComfyUIInputType::String,
                    default_value: Some("".to_string()),
                    required: true,
                    description: "Style description or reference".to_string(),
                },
                ComfyUIInput {
                    name: "style_strength".to_string(),
                    input_type: ComfyUIInputType::Float,
                    default_value: Some("0.75".to_string()),
                    required: false,
                    description: "Style transfer strength (0.0-1.0)".to_string(),
                },
            ],
            workflow_json: HashMap::new(),
        }
    }

    /// Inpainting template
    fn inpainting() -> ComfyUIWorkflowTemplate {
        ComfyUIWorkflowTemplate {
            name: "Inpainting".to_string(),
            description: "Fill in or modify specific regions of an image".to_string(),
            category: "Image Editing".to_string(),
            required_inputs: vec![
                ComfyUIInput {
                    name: "image".to_string(),
                    input_type: ComfyUIInputType::Image,
                    default_value: None,
                    required: true,
                    description: "Original image".to_string(),
                },
                ComfyUIInput {
                    name: "mask".to_string(),
                    input_type: ComfyUIInputType::Image,
                    default_value: None,
                    required: true,
                    description: "Mask defining the region to inpaint".to_string(),
                },
                ComfyUIInput {
                    name: "prompt".to_string(),
                    input_type: ComfyUIInputType::String,
                    default_value: Some("".to_string()),
                    required: true,
                    description: "What to generate in the masked region".to_string(),
                },
            ],
            workflow_json: HashMap::new(),
        }
    }

    /// Face restoration template
    fn face_restore() -> ComfyUIWorkflowTemplate {
        ComfyUIWorkflowTemplate {
            name: "Face Restore".to_string(),
            description: "Restore and enhance faces in images using CodeFormer or GFPGAN".to_string(),
            category: "Image Enhancement".to_string(),
            required_inputs: vec![
                ComfyUIInput {
                    name: "image".to_string(),
                    input_type: ComfyUIInputType::Image,
                    default_value: None,
                    required: true,
                    description: "Image containing faces to restore".to_string(),
                },
                ComfyUIInput {
                    name: "restore_level".to_string(),
                    input_type: ComfyUIInputType::Float,
                    default_value: Some("0.7".to_string()),
                    required: false,
                    description: "Restoration strength (0.0-1.0)".to_string(),
                },
            ],
            workflow_json: HashMap::new(),
        }
    }

    /// Video generation template (AnimateDiff/SVD)
    fn video_generation() -> ComfyUIWorkflowTemplate {
        ComfyUIWorkflowTemplate {
            name: "Video Generation".to_string(),
            description: "Generate short video clips from text or image prompts".to_string(),
            category: "Video".to_string(),
            required_inputs: vec![
                ComfyUIInput {
                    name: "prompt".to_string(),
                    input_type: ComfyUIInputType::String,
                    default_value: Some("".to_string()),
                    required: true,
                    description: "Video content description".to_string(),
                },
                ComfyUIInput {
                    name: "frame_count".to_string(),
                    input_type: ComfyUIInputType::Integer,
                    default_value: Some("16".to_string()),
                    required: false,
                    description: "Number of frames to generate".to_string(),
                },
                ComfyUIInput {
                    name: "fps".to_string(),
                    input_type: ComfyUIInputType::Integer,
                    default_value: Some("8".to_string()),
                    required: false,
                    description: "Frames per second".to_string(),
                },
                ComfyUIInput {
                    name: "motion_scale".to_string(),
                    input_type: ComfyUIInputType::Float,
                    default_value: Some("1.0".to_string()),
                    required: false,
                    description: "Motion intensity (0.5-2.0)".to_string(),
                },
            ],
            workflow_json: HashMap::new(),
        }
    }

    /// Batch processing template
    fn batch_processing() -> ComfyUIWorkflowTemplate {
        ComfyUIWorkflowTemplate {
            name: "Batch Processing".to_string(),
            description: "Process multiple images with the same settings".to_string(),
            category: "Utility".to_string(),
            required_inputs: vec![
                ComfyUIInput {
                    name: "prompt".to_string(),
                    input_type: ComfyUIInputType::String,
                    default_value: Some("".to_string()),
                    required: true,
                    description: "Base prompt for batch generation".to_string(),
                },
                ComfyUIInput {
                    name: "batch_size".to_string(),
                    input_type: ComfyUIInputType::Integer,
                    default_value: Some("4".to_string()),
                    required: false,
                    description: "Number of images to generate".to_string(),
                },
                ComfyUIInput {
                    name: "seed_variation".to_string(),
                    input_type: ComfyUIInputType::Boolean,
                    default_value: Some("true".to_string()),
                    required: false,
                    description: "Vary seed for each image".to_string(),
                },
            ],
            workflow_json: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comfyui_config_default() {
        let config = ComfyUIConfig::default();
        assert_eq!(config.server_url, "http://127.0.0.1:8188");
        assert!(config.auto_reconnect);
    }

    #[test]
    fn test_comfyui_config_custom_url() {
        let config = ComfyUIConfig::new("http://192.168.1.100:8188".to_string());
        assert_eq!(config.server_url, "http://192.168.1.100:8188");
        assert_eq!(config.websocket_url, "ws://192.168.1.100:8188/ws");
    }

    #[test]
    fn test_comfyui_config_validation() {
        let config = ComfyUIConfig::default();
        assert!(config.validate().is_ok());

        let invalid_config = ComfyUIConfig {
            server_url: "".to_string(),
            ..Default::default()
        };
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_template_library() {
        let templates = ComfyUITemplateLibrary::get_templates();
        assert!(!templates.is_empty());
        assert!(templates.iter().any(|t| t.name == "Basic Text-to-Image"));
        assert!(templates.iter().any(|t| t.name == "Upscale & Enhance"));
        assert!(templates.iter().any(|t| t.name == "Style Transfer"));
        assert!(templates.iter().any(|t| t.name == "Inpainting"));
        assert!(templates.iter().any(|t| t.name == "Face Restore"));
        assert!(templates.iter().any(|t| t.name == "Video Generation"));
        assert!(templates.iter().any(|t| t.name == "Batch Processing"));
        assert_eq!(templates.len(), 9);
    }
}
