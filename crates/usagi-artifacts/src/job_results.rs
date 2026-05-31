use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::Serialize;
use usagi_common::error::{ErrorCode, Result, UsagiError};
use usagi_common::manifest::{sha256_file, ArtifactManifest};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct JobResultArtifactOptions {
    pub results_root: PathBuf,
    pub job_id: String,
    pub artifact_id: Option<String>,
    pub provenance: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JobResultArtifact {
    pub artifact_id: String,
    pub path: String,
    pub manifest_path: String,
    pub content_type: String,
    pub sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<serde_json::Value>,
}

pub fn write_mapper_batch_results<I, T>(
    options: JobResultArtifactOptions,
    rows: I,
) -> Result<JobResultArtifact>
where
    I: IntoIterator<Item = T>,
    T: Serialize,
{
    let final_dir = options.results_root.join(&options.job_id);
    if final_dir.exists() {
        return Err(UsagiError::new(
            ErrorCode::IncompatibleArtifact,
            format!(
                "job result artifact already exists at {}",
                final_dir.display()
            ),
        ));
    }
    std::fs::create_dir_all(&options.results_root)?;
    let temp_dir = options.results_root.join(format!(
        ".{}.tmp.{}",
        options.job_id,
        Uuid::new_v4().simple()
    ));
    std::fs::create_dir(&temp_dir)?;

    let write_result = write_mapper_batch_results_inner(&temp_dir, &options, rows);
    match write_result {
        Ok(mut artifact) => {
            std::fs::rename(&temp_dir, &final_dir)?;
            artifact.path = final_dir.join("results.jsonl").display().to_string();
            artifact.manifest_path = final_dir.join("manifest.json").display().to_string();
            Ok(artifact)
        }
        Err(err) => {
            let _ = std::fs::remove_dir_all(&temp_dir);
            Err(err)
        }
    }
}

pub fn read_job_result_artifact(results_path: impl AsRef<Path>) -> Result<JobResultArtifact> {
    let results_path = results_path.as_ref();
    let manifest_path = results_path
        .parent()
        .ok_or_else(|| UsagiError::incompatible_artifact("job result path has no parent"))?
        .join("manifest.json");
    let manifest: ArtifactManifest = serde_json::from_slice(&std::fs::read(&manifest_path)?)?;
    let output = manifest
        .outputs
        .iter()
        .find(|item| item.path == "results.jsonl")
        .ok_or_else(|| {
            UsagiError::incompatible_artifact("job result manifest is missing results.jsonl")
        })?;
    let sha256 = output.sha256.clone().ok_or_else(|| {
        UsagiError::incompatible_artifact("job result manifest is missing sha256")
    })?;
    let actual = sha256_file(results_path)?;
    if actual != sha256 {
        return Err(
            UsagiError::incompatible_artifact("job result artifact checksum mismatch")
                .with_details(serde_json::json!({"expected": sha256, "actual": actual})),
        );
    }
    Ok(JobResultArtifact {
        artifact_id: manifest.artifact_id,
        path: results_path.display().to_string(),
        manifest_path: manifest_path.display().to_string(),
        content_type: output
            .content_type
            .clone()
            .unwrap_or_else(|| "application/jsonl".to_string()),
        sha256,
        provenance: manifest
            .extra
            .as_ref()
            .and_then(|extra| extra.get("provenance").cloned()),
    })
}

pub fn job_results_response(
    job_id: &str,
    state: impl Serialize,
    inline_result: Option<serde_json::Value>,
    error: Option<serde_json::Value>,
    artifact_path: Option<String>,
) -> Result<serde_json::Value> {
    if let Some(path) = artifact_path {
        let artifact = read_job_result_artifact(path)?;
        return Ok(serde_json::json!({
            "job_id": job_id,
            "state": state,
            "artifact": artifact
        }));
    }
    Ok(serde_json::json!({
        "job_id": job_id,
        "state": state,
        "result": inline_result,
        "error": error
    }))
}

fn write_mapper_batch_results_inner<I, T>(
    temp_dir: &Path,
    options: &JobResultArtifactOptions,
    rows: I,
) -> Result<JobResultArtifact>
where
    I: IntoIterator<Item = T>,
    T: Serialize,
{
    let results_path = temp_dir.join("results.jsonl");
    let file = File::create(&results_path)?;
    let mut writer = BufWriter::new(file);
    for row in rows {
        serde_json::to_writer(&mut writer, &row)?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;

    let sha256 = sha256_file(&results_path)?;
    let artifact_id = options
        .artifact_id
        .clone()
        .unwrap_or_else(|| format!("{}_results_v1", options.job_id));
    let mut manifest = serde_json::json!({
        "artifact_id": artifact_id,
        "artifact_kind": "mapper-batch-results",
        "schema_version": "usagi-mapper-results-v1",
        "api_version": "0.1.0",
        "created_at": Utc::now().to_rfc3339(),
        "job_id": options.job_id,
        "outputs": [
            {
                "path": "results.jsonl",
                "content_type": "application/jsonl",
                "sha256": sha256
            }
        ]
    });
    if let Some(provenance) = &options.provenance {
        manifest["extra"] = serde_json::json!({
            "provenance": provenance
        });
    }
    let manifest_path = temp_dir.join("manifest.json");
    std::fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;

    Ok(JobResultArtifact {
        artifact_id: options
            .artifact_id
            .clone()
            .unwrap_or_else(|| format!("{}_results_v1", options.job_id)),
        path: results_path.display().to_string(),
        manifest_path: manifest_path.display().to_string(),
        content_type: "application/jsonl".to_string(),
        sha256,
        provenance: options.provenance.clone(),
    })
}
