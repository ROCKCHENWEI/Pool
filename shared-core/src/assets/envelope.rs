use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

use crate::engine::PoolRuntimePlan;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEnvelopeManifest {
    pub root: String,
    pub project_json: String,
    pub workflow_json: String,
    pub scene_json: String,
    pub source_dir: String,
    pub output_dir: String,
    pub requests_dir: String,
    pub control_dir: String,
}

pub fn materialize_project_envelope(
    base_dir: impl AsRef<Path>,
    plan: &PoolRuntimePlan,
) -> Result<ProjectEnvelopeManifest> {
    let base_dir = base_dir.as_ref();
    let envelope = &plan.envelope;

    let root = base_dir.join(&envelope.root);
    let source_dir = base_dir.join(&envelope.source_dir);
    let output_dir = base_dir.join(&envelope.output_dir);
    let requests_dir = base_dir.join(&envelope.requests_dir);
    let control_dir = base_dir.join(&envelope.control_dir);

    for dir in [&root, &source_dir, &output_dir, &requests_dir, &control_dir] {
        fs::create_dir_all(dir)
            .with_context(|| format!("create envelope dir {}", dir.display()))?;
    }

    write_json(
        base_dir.join(&envelope.project_json),
        &json!({
            "project": plan.project,
            "envelope": envelope,
            "providers": plan.providers,
            "software_adapters": plan.software_adapters,
        }),
    )?;
    write_json(
        base_dir.join(&envelope.workflow_json),
        &json!({
            "workflow": plan.workflow,
            "shots": plan.shots,
        }),
    )?;
    write_json(
        base_dir.join(&envelope.scene_json),
        &json!({
            "project_slug": plan.project.slug,
            "assembly_target": "unreal",
            "local_asset_contract": "image-blaster-indexed-files",
            "outputs": ["video", "game", "interactive_art"],
        }),
    )?;

    Ok(ProjectEnvelopeManifest {
        root: path_string(root),
        project_json: path_string(base_dir.join(&envelope.project_json)),
        workflow_json: path_string(base_dir.join(&envelope.workflow_json)),
        scene_json: path_string(base_dir.join(&envelope.scene_json)),
        source_dir: path_string(source_dir),
        output_dir: path_string(output_dir),
        requests_dir: path_string(requests_dir),
        control_dir: path_string(control_dir),
    })
}

fn write_json(path: PathBuf, value: &serde_json::Value) -> Result<()> {
    let body = serde_json::to_string_pretty(value).context("serialize envelope JSON")?;
    fs::write(&path, body).with_context(|| format!("write envelope JSON {}", path.display()))
}

fn path_string(path: PathBuf) -> String {
    path.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::build_default_content_burst_plan;

    #[test]
    fn materializes_image_blaster_style_project_envelope() {
        let base =
            std::env::temp_dir().join(format!("pool-envelope-test-{}", uuid::Uuid::new_v4()));
        let plan = build_default_content_burst_plan("demo", "Pool demo");

        let manifest = materialize_project_envelope(&base, &plan).unwrap();

        assert!(Path::new(&manifest.project_json).exists());
        assert!(Path::new(&manifest.workflow_json).exists());
        assert!(Path::new(&manifest.scene_json).exists());
        assert!(Path::new(&manifest.source_dir).is_dir());
        assert!(Path::new(&manifest.output_dir).is_dir());
        assert!(Path::new(&manifest.requests_dir).is_dir());
        assert!(Path::new(&manifest.control_dir).is_dir());

        fs::remove_dir_all(base).unwrap();
    }
}
