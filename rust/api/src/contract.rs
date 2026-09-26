use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Provenance {
    pub git_commit: String,
    pub config_hash: String,
    pub data_version: String,
    pub model_artifact_id: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Instrument {
    pub id: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RunManifest {
    pub schema_version: u32,
    pub run_id: String,
    pub created_at: String,
    pub kind: String,
    /// test, validation, train, or unverified. A label alone is not proof of OOS.
    pub split: String,
    pub provenance: Provenance,
    pub instruments: Vec<Instrument>,
    pub capabilities: Vec<String>,
    pub artifacts: BTreeMap<String, String>,
    pub metrics: BTreeMap<String, f64>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ModelArtifact {
    pub artifact_id: String,
    pub metadata: serde_json::Value,
    pub training_log: Option<serde_json::Value>,
    pub validation: Option<serde_json::Value>,
    pub onnx_present: bool,
}
