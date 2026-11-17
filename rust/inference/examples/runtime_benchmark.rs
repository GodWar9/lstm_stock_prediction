//! Runtime-only timing on fixed, already-scaled package parity windows.
use anyhow::{ensure, Context, Result};
use quant_inference::{FittedScaler, ModelMetadata, OnnxSession};
use serde_json::{json, Value};
use std::{fs, path::PathBuf, time::Instant};

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let dir = PathBuf::from(args.get(1).context("artifact directory required")?);
    let runs: usize = args.get(2).context("run count required")?.parse()?;
    ensure!(runs >= 10_000, "At least 10000 measured runs required");
    quant_inference::integrity::verify_package(&dir)?;
    let metadata = ModelMetadata::load(dir.join("metadata.json"))?;
    let scaler = FittedScaler::load(dir.join("scaler.json"))?;
    let validation: Value = serde_json::from_slice(&fs::read(dir.join("validation.json"))?)?;
    let cases = validation["runtime_parity"]["cases"]
        .as_array()
        .context("parity windows required")?;
    ensure!(!cases.is_empty(), "Empty parity windows");
    let windows: Vec<Vec<f32>> = cases
        .iter()
        .map(|case| {
            let raw: Vec<f64> = serde_json::from_value(case["raw_features"].clone())?;
            Ok(scaler.transform_f32(&raw)?)
        })
        .collect::<Result<_>>()?;
    let session = OnnxSession::load(
        dir.join("model.onnx"),
        &metadata.model_id,
        metadata.lookback,
        scaler.num_features(),
    )?;
    let predict = |i: usize| {
        session.predict_f32_slice(
            &windows[i % windows.len()],
            metadata.lookback,
            scaler.num_features(),
        )
    };
    for i in 0..100 {
        std::hint::black_box(predict(i)?);
    }
    let mut samples = Vec::with_capacity(runs);
    for i in 0..runs {
        let start = Instant::now();
        std::hint::black_box(predict(i)?);
        samples.push(start.elapsed().as_secs_f64() * 1e6);
    }
    let predictions: Vec<f64> = (0..windows.len())
        .map(|i| Ok(predict(i)?.value))
        .collect::<Result<_>>()?;
    println!(
        "{}",
        json!({"runtime":"tract-onnx","samples_us":samples,"predictions":predictions,"warmup":100,
        "profile":if cfg!(debug_assertions) {"debug"} else {"release"}})
    );
    Ok(())
}
