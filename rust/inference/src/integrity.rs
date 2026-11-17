//! Verify the exact bytes of a published model package before loading it.
use crate::provider::InferenceError;
use ring::signature::{Ed25519KeyPair, KeyPair, UnparsedPublicKey, ED25519};
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
    let policy = match std::env::var("QUANTCTL_MODEL_TRUST_CONFIG") {
        Ok(path) => serde_yaml::from_slice(&fs::read(path)?)
            .map_err(|_| invalid("Invalid model trust configuration"))?,
        Err(std::env::VarError::NotPresent) => TrustPolicy::default(),
        Err(_) => return Err(invalid("Invalid trust configuration path")),
    };
    verify_package_with_policy(directory, &policy)
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustPolicy {
    #[serde(default)]
    pub require_signature: bool,
    pub public_key_hex: Option<String>,
}

fn decode_hex(value: &str) -> Result<Vec<u8>, InferenceError> {
    if !value.len().is_multiple_of(2) || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(invalid("Invalid hex key encoding"));
    }
    (0..value.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&value[i..i + 2], 16)
                .map_err(|_| invalid("Invalid hex key encoding"))
        })
        .collect()
}

pub fn verify_package_with_policy(
    directory: &Path,
    policy: &TrustPolicy,
) -> Result<String, InferenceError> {
    let bytes = fs::read(directory.join("integrity.json")).map_err(|e| {
        invalid(format!(
            "Cannot read integrity.json; retrain this model: {e}"
        ))
    })?;
    match fs::read(directory.join("integrity.ed25519")) {
        Ok(signature) => {
            let key = decode_hex(
                policy
                    .public_key_hex
                    .as_deref()
                    .ok_or_else(|| invalid("Signed model requires a trusted public key"))?,
            )?;
            UnparsedPublicKey::new(&ED25519, key)
                .verify(&bytes, &signature)
                .map_err(|_| invalid("Ed25519 model signature verification failed"))?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !policy.require_signature => {
        }
        Err(_) => return Err(invalid("Required model signature missing or unreadable")),
    }
    verify_package_hashes(directory)
}

/// Signing checks hashes independently of the deployment trust policy.
pub fn sign_package(directory: &Path) -> Result<String, InferenceError> {
    verify_package_hashes(directory)?;
    let encoded = std::env::var("QUANTCTL_MODEL_SIGNING_KEY_PKCS8_HEX")
        .map_err(|_| invalid("Signing key must be supplied through the process environment"))?;
    let mut secret = decode_hex(&encoded)?;
    let key_result = Ed25519KeyPair::from_pkcs8(&secret);
    secret.fill(0);
    let key = key_result.map_err(|_| invalid("Invalid Ed25519 PKCS8 v2 signing key"))?;
    let signature = key.sign(&fs::read(directory.join("integrity.json"))?);
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(directory.join("integrity.ed25519"))?;
    use std::io::Write;
    file.write_all(signature.as_ref())?;
    file.sync_all()?;
    Ok(key
        .public_key()
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

fn verify_package_hashes(directory: &Path) -> Result<String, InferenceError> {
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

    #[test]
    fn signature_policy_accepts_valid_and_rejects_unsigned_tampered_and_wrong_key() {
        let temp = package();
        let key = Ed25519KeyPair::from_seed_unchecked(&[7; 32]).unwrap();
        let public: String = key
            .public_key()
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let policy = TrustPolicy {
            require_signature: true,
            public_key_hex: Some(public),
        };
        assert!(verify_package_with_policy(temp.path(), &policy).is_err());
        assert!(verify_package_with_policy(temp.path(), &TrustPolicy::default()).is_ok());
        let manifest = fs::read(temp.path().join("integrity.json")).unwrap();
        fs::write(
            temp.path().join("integrity.ed25519"),
            key.sign(&manifest).as_ref(),
        )
        .unwrap();
        assert!(verify_package_with_policy(temp.path(), &policy).is_ok());
        assert!(verify_package_with_policy(temp.path(), &TrustPolicy::default()).is_err());
        let other = Ed25519KeyPair::from_seed_unchecked(&[8; 32]).unwrap();
        let wrong = TrustPolicy {
            require_signature: true,
            public_key_hex: Some(
                other
                    .public_key()
                    .as_ref()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect(),
            ),
        };
        assert!(verify_package_with_policy(temp.path(), &wrong).is_err());
        fs::write(
            temp.path().join("integrity.json"),
            [manifest.as_slice(), b" "].concat(),
        )
        .unwrap();
        assert!(verify_package_with_policy(temp.path(), &policy).is_err());
        fs::write(temp.path().join("integrity.json"), manifest).unwrap();
        fs::write(temp.path().join("model.onnx"), b"modified").unwrap();
        assert!(verify_package_with_policy(temp.path(), &policy).is_err());
    }

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
