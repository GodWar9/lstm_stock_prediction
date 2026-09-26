//! Verify the exact bytes of a published model package before loading it.
use crate::provider::InferenceError;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, io::Read, path::Path};

const REQUIRED: [&str; 6] = [
    "model.onnx",
    "model.pt",
    "scaler.json",
    "metadata.json",
    "training_log.json",
    "validation.json",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileDigest {
    sha256: String,
    size_bytes: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    algorithm: String,
    model_id: String,
    files: BTreeMap<String, FileDigest>,
}

fn invalid(message: impl Into<String>) -> InferenceError {
    InferenceError::IntegrityFailed(message.into())
}

/// Returns the verified manifest's model ID. Unsealed legacy packages must be retrained.
pub fn verify_package(directory: &Path) -> Result<String, InferenceError> {
    let bytes = fs::read(directory.join("integrity.json")).map_err(|e| {
        invalid(format!(
            "Cannot read integrity.json; retrain this model: {e}"
        ))
    })?;
    let manifest: Manifest = serde_json::from_slice(&bytes)
        .map_err(|e| invalid(format!("Invalid integrity.json: {e}")))?;
    if manifest.schema_version != 1 || manifest.algorithm != "sha256" {
        return Err(invalid("Unsupported model integrity schema or algorithm"));
    }
    if manifest.model_id.is_empty()
        || manifest.files.len() != REQUIRED.len()
        || REQUIRED
            .iter()
            .any(|name| !manifest.files.contains_key(*name))
    {
        return Err(invalid(
            "Integrity manifest must identify the model and all six package files",
        ));
    }
    for name in REQUIRED {
        let expected = &manifest.files[name];
        if expected.sha256.len() != 64
            || !expected
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(invalid(format!("Invalid SHA-256 for {name}")));
        }
        let path = directory.join(name);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|e| invalid(format!("Missing package file {name}: {e}")))?;
        if !metadata.is_file() || metadata.len() != expected.size_bytes || metadata.len() == 0 {
            return Err(invalid(format!(
                "Package file type or size mismatch: {name}"
            )));
        }
        let mut file =
            fs::File::open(&path).map_err(|e| invalid(format!("Cannot open {name}: {e}")))?;
        let mut digest = Sha256::new();
        let mut buffer = [0_u8; 65536];
        loop {
            let count = file
                .read(&mut buffer)
                .map_err(|e| invalid(format!("Cannot hash {name}: {e}")))?;
            if count == 0 {
                break;
            }
            digest.update(&buffer[..count]);
        }
        if format!("{:x}", digest.finalize()) != expected.sha256 {
            return Err(invalid(format!("SHA-256 mismatch: {name}")));
        }
    }
    Ok(manifest.model_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn package() -> tempfile::TempDir {
        let temp = tempfile::tempdir().unwrap();
        let mut files = serde_json::Map::new();
        for name in REQUIRED {
            fs::write(temp.path().join(name), b"original").unwrap();
            files.insert(
                name.into(),
                json!({
                    "sha256": format!("{:x}", Sha256::digest(b"original")),
                    "size_bytes": 8,
                }),
            );
        }
        fs::write(
            temp.path().join("integrity.json"),
            serde_json::to_vec(&json!({
                "schema_version": 1, "algorithm": "sha256", "model_id": "example", "files": files,
            }))
            .unwrap(),
        )
        .unwrap();
        temp
    }

    #[test]
    fn verifies_package_and_rejects_same_size_mutation() {
        let temp = package();
        assert_eq!(verify_package(temp.path()).unwrap(), "example");
        fs::write(temp.path().join("scaler.json"), b"modified").unwrap();
        assert!(verify_package(temp.path())
            .unwrap_err()
            .to_string()
            .contains("SHA-256 mismatch"));
    }

    #[test]
    fn rejects_missing_manifest_or_package_member() {
        let temp = package();
        fs::remove_file(temp.path().join("model.onnx")).unwrap();
        assert!(verify_package(temp.path()).is_err());
        fs::remove_file(temp.path().join("integrity.json")).unwrap();
        assert!(verify_package(temp.path())
            .unwrap_err()
            .to_string()
            .contains("retrain"));
    }

    #[test]
    fn rejects_invalid_manifests() {
        for change in ["missing", "extra", "algorithm", "schema", "digest"] {
            let temp = package();
            let path = temp.path().join("integrity.json");
            let mut value: serde_json::Value =
                serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            match change {
                "missing" => {
                    value["files"]
                        .as_object_mut()
                        .unwrap()
                        .remove("scaler.json");
                }
                "extra" => {
                    value["files"]["../outside"] = value["files"]["scaler.json"].clone();
                }
                "algorithm" => value["algorithm"] = json!("md5"),
                "schema" => value["schema_version"] = json!(2),
                _ => value["files"]["scaler.json"]["sha256"] = json!("invalid"),
            }
            fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
            assert!(verify_package(temp.path()).is_err(), "accepted {change}");
        }
    }
}
