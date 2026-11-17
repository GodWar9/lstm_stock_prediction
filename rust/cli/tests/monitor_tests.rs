//! Synthetic protocol fixtures only; no investment performance measurements.
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{fs, process::Command};

#[test]
fn monitor_cli_warns_and_exits_nonzero_on_injected_drift() {
    let tmp = tempfile::tempdir().unwrap();
    let model = tmp.path().join("model");
    fs::create_dir(&model).unwrap();
    let metadata = json!({"feature_schema":["x"],"monitor_reference":[[0.],[1.],[2.],[3.]]});
    let mut files = serde_json::Map::new();
    for name in [
        "model.onnx",
        "model.pt",
        "scaler.json",
        "metadata.json",
        "training_log.json",
        "validation.json",
    ] {
        let bytes = if name == "metadata.json" {
            serde_json::to_vec(&metadata).unwrap()
        } else {
            b"synthetic test placeholder".to_vec()
        };
        fs::write(model.join(name), &bytes).unwrap();
        files.insert(
            name.into(),
            json!({"sha256":format!("{:x}",Sha256::digest(&bytes)),"size_bytes":bytes.len()}),
        );
    }
    let manifest = serde_json::to_vec(
        &json!({"schema_version":1,"algorithm":"sha256","model_id":"fixture","files":files}),
    )
    .unwrap();
    fs::write(model.join("integrity.json"), &manifest).unwrap();
    for (offset, expected) in [(1., "warn"), (100., "halt")] {
        let log = tmp.path().join(format!("{expected}.ndjson"));
        for i in 0..4 {
            let id=quant_api::decisions::append(&log,json!({"kind":"forecast","model":"fixture","dataset":"fixture",
                "as_of_ms":i+1,"predicted_log_return":i as f64 * 0.1,"data_provenance":"recorded_real",
                "features":[i as f64 + offset],"feature_schema_sha256":quant_api::decisions::digest(&metadata["feature_schema"]).unwrap(),
                "model_package_sha256":format!("{:x}",Sha256::digest(&manifest))})).unwrap();
            quant_api::decisions::append(
                &log,
                json!({"kind":"outcome","decision_id":id,"observed_at_ms":i+100,
                "realized_log_return":i as f64 * 0.2}),
            )
            .unwrap();
        }
        let result = Command::new(env!("CARGO_BIN_EXE_quantctl"))
            .args(["monitor", "--log"])
            .arg(&log)
            .arg("--model-dir")
            .arg(&model)
            .args(["--window", "4", "--warn-ks", "0.2", "--halt-ks", "0.5"])
            .output()
            .unwrap();
        let stdout = String::from_utf8(result.stdout).unwrap();
        let report: serde_json::Value = stdout
            .lines()
            .rev()
            .find_map(|line| serde_json::from_str(line).ok())
            .expect("monitor JSON");
        assert_eq!(report["status"], expected);
        assert_eq!(report["outcomes"], 4);
        assert_eq!(report["rolling_ic"], 1.);
        assert_eq!(result.status.success(), expected == "warn");
    }
}
