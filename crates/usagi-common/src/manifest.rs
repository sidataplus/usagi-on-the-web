use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{ErrorCode, Result, UsagiError};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactManifest {
    pub artifact_id: String,
    pub artifact_kind: String,
    pub schema_version: String,
    pub api_version: String,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<ManifestFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<ManifestFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestFile {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
}

pub fn sha256_file(path: impl AsRef<Path>) -> Result<String> {
    let path = path.as_ref();
    let file = File::open(path).map_err(|err| {
        UsagiError::new(
            ErrorCode::NotFound,
            format!("failed to open {}: {err}", path.display()),
        )
    })?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

pub fn verify_file_sha256(path: impl AsRef<Path>, expected: &str) -> Result<()> {
    let actual = sha256_file(path.as_ref())?;
    if actual == expected {
        Ok(())
    } else {
        Err(
            UsagiError::incompatible_artifact("artifact checksum mismatch")
                .with_details(serde_json::json!({ "expected": expected, "actual": actual })),
        )
    }
}
