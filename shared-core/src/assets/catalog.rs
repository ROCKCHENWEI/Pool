use std::path::Path;

use crate::models::{AssetRecord, AssetStatus};

pub fn build_asset_records(
    project_slug: &str,
    source_node_id: Option<&str>,
    provider_url: Option<&str>,
    local_paths: &[String],
) -> Vec<AssetRecord> {
    local_paths
        .iter()
        .map(|local_path| {
            let path = Path::new(local_path);
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(local_path)
                .to_string();
            AssetRecord {
                id: uuid::Uuid::new_v4().to_string(),
                project_slug: project_slug.to_string(),
                name,
                asset_type: infer_asset_type(path).to_string(),
                local_path: local_path.to_string(),
                source_node_id: source_node_id.map(ToString::to_string),
                provider_url: provider_url.map(ToString::to_string),
                status: AssetStatus::Local,
                created_at: chrono::Utc::now(),
            }
        })
        .collect()
}

pub fn infer_asset_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png" | "jpg" | "jpeg" | "webp" | "tif" | "tiff" | "exr") => "image",
        Some("mp4" | "mov" | "mkv" | "webm" | "avi") => "video",
        Some("wav" | "mp3" | "flac" | "aiff" | "ogg") => "audio",
        Some("glb" | "gltf" | "fbx" | "obj" | "usd" | "usdz" | "spz" | "ply" | "splat") => "3d",
        Some("json") => "metadata",
        Some("txt" | "md" | "rtf") => "text",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infers_common_media_asset_types() {
        assert_eq!(infer_asset_type(Path::new("frame.png")), "image");
        assert_eq!(infer_asset_type(Path::new("clip.mp4")), "video");
        assert_eq!(infer_asset_type(Path::new("loop.wav")), "audio");
        assert_eq!(infer_asset_type(Path::new("world.spz")), "3d");
        assert_eq!(infer_asset_type(Path::new("request.json")), "metadata");
    }

    #[test]
    fn builds_asset_records_from_local_paths() {
        let paths = vec![
            "worlds/demo/output/1-plate.png".to_string(),
            "worlds/demo/output/2-preview.mp4".to_string(),
        ];
        let assets =
            build_asset_records("demo", Some("node-1"), Some("provider://history"), &paths);

        assert_eq!(assets.len(), 2);
        assert_eq!(assets[0].project_slug, "demo");
        assert_eq!(assets[0].name, "1-plate.png");
        assert_eq!(assets[0].asset_type, "image");
        assert_eq!(assets[0].source_node_id.as_deref(), Some("node-1"));
        assert_eq!(
            assets[0].provider_url.as_deref(),
            Some("provider://history")
        );
        assert_eq!(assets[1].asset_type, "video");
    }
}
