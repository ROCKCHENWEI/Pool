use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

pub fn local_input_manifest(input_paths: &[String], context: &str) -> Result<Value> {
    reject_remote_input_paths(input_paths, context)?;
    let entries = input_paths
        .iter()
        .map(|path| local_input_entry(path))
        .collect::<Result<Vec<_>>>()?;
    Ok(Value::Array(entries))
}

pub fn reject_remote_input_paths(input_paths: &[String], context: &str) -> Result<()> {
    for input_path in input_paths {
        let trimmed = input_path.trim();
        if trimmed.is_empty() {
            bail!("{context} input_paths cannot contain empty paths");
        }
        if is_remote_or_inline_path(trimmed) {
            bail!("{context} input_paths must be local file paths: {trimmed}");
        }
    }
    Ok(())
}

fn local_input_entry(input_path: &str) -> Result<Value> {
    let path = Path::new(input_path);
    let metadata = match fs::metadata(path) {
        Ok(metadata) => Some(metadata),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(error)
                .with_context(|| format!("read local input metadata {}", path.display()));
        }
    };
    let absolute_path = if metadata.is_some() {
        Some(
            fs::canonicalize(path)
                .with_context(|| format!("canonicalize local input {}", path.display()))?
                .to_string_lossy()
                .to_string(),
        )
    } else if path.is_absolute() {
        Some(input_path.to_string())
    } else {
        None
    };

    Ok(json!({
        "path": input_path,
        "absolute_path": absolute_path,
        "file_name": path.file_name().and_then(|value| value.to_str()),
        "extension": path.extension().and_then(|value| value.to_str()).map(|value| value.to_ascii_lowercase()),
        "mime_type": mime_type_for_path(path),
        "bytes": metadata.as_ref().map(|metadata| metadata.len()),
        "exists": metadata.is_some(),
    }))
}

fn is_remote_or_inline_path(value: &str) -> bool {
    value.starts_with("http://")
        || value.starts_with("https://")
        || value.starts_with("s3://")
        || value.starts_with("gs://")
        || value.starts_with("data:")
}

fn mime_type_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "glb" => "model/gltf-binary",
        "gltf" => "model/gltf+json",
        "json" => "application/json",
        "spz" => "application/octet-stream",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_manifest_without_reading_file_bytes() {
        let root = std::env::temp_dir().join(format!(
            "pool-local-input-manifest-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).unwrap();
        let image_path = root.join("plate.png");
        fs::write(&image_path, b"fake-image").unwrap();

        let manifest =
            local_input_manifest(&[image_path.to_string_lossy().to_string()], "media").unwrap();

        assert_eq!(manifest[0]["exists"], true);
        assert_eq!(manifest[0]["file_name"], "plate.png");
        assert_eq!(manifest[0]["mime_type"], "image/png");
        assert_eq!(manifest[0]["bytes"], 10);
        assert!(manifest.to_string().find("fake-image").is_none());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_remote_or_inline_inputs() {
        let error =
            reject_remote_input_paths(&["https://example.com/reference.png".to_string()], "3DGS")
                .unwrap_err();

        assert!(error.to_string().contains("local file paths"));
    }
}
