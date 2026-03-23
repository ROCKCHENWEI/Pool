//! Automatic1111 (Stable Diffusion WebUI) Adapter
//!
//! Provides integration with Automatic1111's Stable Diffusion WebUI API
//! for local image generation.

use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Automatic1111 WebUI adapter for local image generation
pub struct Automatic1111Adapter {
    client: Client,
    base_url: String,
}

impl Automatic1111Adapter {
    /// Create a new adapter with the given base URL
    pub fn new(base_url: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .unwrap_or_default();

        // Remove trailing slash
        let base_url = base_url.trim_end_matches('/').to_string();

        Self { client, base_url }
    }

    /// Check if the API is accessible
    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/sdapi/v1/sd-models", self.base_url);
        let response = self.client.get(&url).send().await?;
        Ok(response.status().is_success())
    }

    /// Get available checkpoints/models
    pub async fn get_models(&self) -> Result<Vec<SDModel>> {
        let url = format!("{}/sdapi/v1/sd-models", self.base_url);
        let response = self.client.get(&url).send().await?;
        let models: Vec<SDModel> = response.json().await?;
        Ok(models)
    }

    /// Get available samplers
    pub async fn get_samplers(&self) -> Result<Vec<Sampler>> {
        let url = format!("{}/sdapi/v1/samplers", self.base_url);
        let response = self.client.get(&url).send().await?;
        let samplers: Vec<Sampler> = response.json().await?;
        Ok(samplers)
    }

    /// Get current options
    pub async fn get_options(&self) -> Result<SDOptions> {
        let url = format!("{}/sdapi/v1/options", self.base_url);
        let response = self.client.get(&url).send().await?;
        let options: SDOptions = response.json().await?;
        Ok(options)
    }

    /// Set current model/checkpoint
    pub async fn set_model(&self, model_name: &str) -> Result<()> {
        let url = format!("{}/sdapi/v1/options", self.base_url);
        let payload = serde_json::json!({ "sd_model_checkpoint": model_name });
        let response = self.client.post(&url).json(&payload).send().await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow!("Failed to set model: {}", response.status()))
        }
    }

    /// Generate image from text prompt (txt2img)
    pub async fn text_to_image(&self, request: Txt2ImgRequest) -> Result<ImageResponse> {
        let url = format!("{}/sdapi/v1/txt2img", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;

        if response.status().is_success() {
            let result: ImageResponse = response.json().await?;
            Ok(result)
        } else {
            let error_text = response.text().await?;
            Err(anyhow!("txt2img failed: {}", error_text))
        }
    }

    /// Generate image from image (img2img)
    pub async fn image_to_image(&self, request: Img2ImgRequest) -> Result<ImageResponse> {
        let url = format!("{}/sdapi/v1/img2img", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;

        if response.status().is_success() {
            let result: ImageResponse = response.json().await?;
            Ok(result)
        } else {
            let error_text = response.text().await?;
            Err(anyhow!("img2img failed: {}", error_text))
        }
    }

    /// Generate image with extra single image (for upscale, etc.)
    pub async fn extra_single_image(&self, request: ExtraSingleImageRequest) -> Result<ImageResponse> {
        let url = format!("{}/sdapi/v1/extra-single-image", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;

        if response.status().is_success() {
            let result: ImageResponse = response.json().await?;
            Ok(result)
        } else {
            let error_text = response.text().await?;
            Err(anyhow!("extra-single-image failed: {}", error_text))
        }
    }

    /// Get progress information
    pub async fn get_progress(&self) -> Result<ProgressResponse> {
        let url = format!("{}/sdapi/v1/progress", self.base_url);
        let response = self.client.get(&url).send().await?;
        let progress: ProgressResponse = response.json().await?;
        Ok(progress)
    }

    /// Interrupt current generation
    pub async fn interrupt(&self) -> Result<()> {
        let url = format!("{}/sdapi/v1/interrupt", self.base_url);
        let response = self.client.post(&url).send().await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow!("Failed to interrupt"))
        }
    }

    /// Get available LoRA models
    pub async fn get_loras(&self) -> Result<Vec<LoraModel>> {
        let url = format!("{}/sdapi/v1/loras", self.base_url);
        let response = self.client.get(&url).send().await?;
        let loras: Vec<LoraModel> = response.json().await?;
        Ok(loras)
    }

    /// Get available embeddings (textual inversion)
    pub async fn get_embeddings(&self) -> Result<EmbeddingsResponse> {
        let url = format!("{}/sdapi/v1/embeddings", self.base_url);
        let response = self.client.get(&url).send().await?;
        let embeddings: EmbeddingsResponse = response.json().await?;
        Ok(embeddings)
    }
}

// ============================================================================
// Data Structures
// ============================================================================

/// Stable Diffusion model/checkpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SDModel {
    pub title: String,
    pub model_name: String,
    pub hash: Option<String>,
    pub sha256: Option<String>,
    pub filename: Option<String>,
    pub config: Option<String>,
}

/// Sampler information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sampler {
    pub name: String,
    pub aliases: Vec<String>,
    pub options: Option<serde_json::Value>,
}

/// SD Options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SDOptions {
    pub sd_model_checkpoint: Option<String>,
    pub sd_vae: Option<String>,
    pub clip_stop_at_last_layers: Option<i32>,
    pub upscaler_for_img2img: Option<String>,
    pub samples_format: Option<String>,
}

/// Text-to-Image request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Txt2ImgRequest {
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub styles: Option<Vec<String>>,
    pub seed: Option<i64>,
    pub subseed: Option<i64>,
    pub subseed_strength: Option<f32>,
    pub seed_resize_from_h: Option<i32>,
    pub seed_resize_from_w: Option<i32>,
    pub sampler_name: Option<String>,
    pub batch_size: Option<i32>,
    pub n_iter: Option<i32>,
    pub steps: Option<i32>,
    pub cfg_scale: Option<f32>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub restore_faces: Option<bool>,
    pub tiling: Option<bool>,
    pub do_not_save_samples: Option<bool>,
    pub do_not_save_grid: Option<bool>,
    pub eta: Option<f32>,
    pub denoising_strength: Option<f32>,
    pub s_min_uncond: Option<f32>,
    pub s_churn: Option<f32>,
    pub s_tmax: Option<f32>,
    pub s_tmin: Option<f32>,
    pub s_noise: Option<f32>,
    pub override_settings: Option<serde_json::Value>,
    pub override_settings_restore_afterwards: Option<bool>,
    pub script_args: Option<Vec<serde_json::Value>>,
    pub sampler_index: Option<String>,
    pub script_name: Option<String>,
    pub save_images: Option<bool>,
    pub send_images: Option<bool>,
    pub alwayson_scripts: Option<serde_json::Value>,
}

impl Txt2ImgRequest {
    /// Create a new request with the given prompt
    pub fn new(prompt: &str) -> Self {
        Self {
            prompt: prompt.to_string(),
            negative_prompt: None,
            styles: None,
            seed: None,
            subseed: None,
            subseed_strength: None,
            seed_resize_from_h: None,
            seed_resize_from_w: None,
            sampler_name: Some("DPM++ 2M Karras".to_string()),
            batch_size: Some(1),
            n_iter: Some(1),
            steps: Some(20),
            cfg_scale: Some(7.0),
            width: Some(512),
            height: Some(512),
            restore_faces: None,
            tiling: None,
            do_not_save_samples: None,
            do_not_save_grid: None,
            eta: None,
            denoising_strength: None,
            s_min_uncond: None,
            s_churn: None,
            s_tmax: None,
            s_tmin: None,
            s_noise: None,
            override_settings: None,
            override_settings_restore_afterwards: None,
            script_args: None,
            sampler_index: None,
            script_name: None,
            save_images: None,
            send_images: Some(true),
            alwayson_scripts: None,
        }
    }

    /// Set negative prompt
    pub fn with_negative_prompt(mut self, negative: &str) -> Self {
        self.negative_prompt = Some(negative.to_string());
        self
    }

    /// Set dimensions
    pub fn with_dimensions(mut self, width: i32, height: i32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }

    /// Set steps
    pub fn with_steps(mut self, steps: i32) -> Self {
        self.steps = Some(steps);
        self
    }

    /// Set seed
    pub fn with_seed(mut self, seed: i64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Set sampler
    pub fn with_sampler(mut self, sampler: &str) -> Self {
        self.sampler_name = Some(sampler.to_string());
        self
    }

    /// Set CFG scale
    pub fn with_cfg_scale(mut self, cfg: f32) -> Self {
        self.cfg_scale = Some(cfg);
        self
    }
}

/// Image-to-Image request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Img2ImgRequest {
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub init_images: Vec<String>,
    pub resize_mode: Option<i32>,
    pub denoising_strength: Option<f32>,
    pub image_cfg_scale: Option<f32>,
    pub mask: Option<String>,
    pub mask_blur: Option<i32>,
    pub inpainting_fill: Option<i32>,
    pub inpaint_full_res: Option<bool>,
    pub inpaint_full_res_padding: Option<i32>,
    pub inpainting_mask_invert: Option<i32>,
    pub initial_noise_multiplier: Option<f32>,
    pub rest: Txt2ImgRequest,
}

/// Extra single image request (for upscaling)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtraSingleImageRequest {
    pub resize_mode: Option<i32>,
    pub show_extras_results: Option<bool>,
    pub gfpgan_visibility: Option<f32>,
    pub codeformer_visibility: Option<f32>,
    pub codeformer_weight: Option<f32>,
    pub upscaling_resize: Option<f32>,
    pub upscaling_resize_w: Option<i32>,
    pub upscaling_resize_h: Option<i32>,
    pub upscaling_crop: Option<bool>,
    pub upscaler_1: Option<String>,
    pub upscaler_2: Option<String>,
    pub extras_upscaler_2_visibility: Option<f32>,
    pub image: Option<String>,
}

/// Image generation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageResponse {
    pub images: Vec<String>,
    pub parameters: Option<serde_json::Value>,
    pub info: Option<String>,
}

/// Progress response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressResponse {
    pub progress: f32,
    pub eta_relative: f32,
    pub state: Option<JobState>,
    pub current_image: Option<String>,
    pub textinfo: Option<String>,
}

/// Job state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobState {
    pub job: Option<String>,
    pub job_count: Option<i32>,
    pub job_timestamp: Option<String>,
    pub job_no: Option<i32>,
    pub sampling_step: Option<i32>,
    pub sampling_steps: Option<i32>,
}

/// LoRA model information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoraModel {
    pub name: String,
    pub alias: Option<String>,
    pub path: Option<String>,
}

/// Embeddings response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingsResponse {
    pub loaded: Option<serde_json::Value>,
    pub skipped: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_txt2img_request_builder() {
        let request = Txt2ImgRequest::new("a beautiful landscape")
            .with_negative_prompt("ugly, blurry")
            .with_dimensions(1024, 768)
            .with_steps(30)
            .with_seed(12345)
            .with_sampler("Euler a")
            .with_cfg_scale(8.0);

        assert_eq!(request.prompt, "a beautiful landscape");
        assert_eq!(request.negative_prompt, Some("ugly, blurry".to_string()));
        assert_eq!(request.width, Some(1024));
        assert_eq!(request.height, Some(768));
        assert_eq!(request.steps, Some(30));
        assert_eq!(request.seed, Some(12345));
        assert_eq!(request.sampler_name, Some("Euler a".to_string()));
        assert_eq!(request.cfg_scale, Some(8.0));
    }

    #[test]
    fn test_adapter_creation() {
        let adapter = Automatic1111Adapter::new("http://127.0.0.1:7860");
        assert_eq!(adapter.base_url, "http://127.0.0.1:7860");
    }
}
