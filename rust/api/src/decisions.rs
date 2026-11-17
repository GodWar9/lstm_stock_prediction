//! Append-only, hash-chained research decisions. Hashes are not signatures.
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

const LIMIT: u64 = 128 * 1024 * 1024;

pub fn digest(value: &Value) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}

fn decode(bytes: &[u8]) -> Result<Vec<Value>> {
    ensure!(
        bytes.is_empty() || bytes.last() == Some(&b'\n'),
        "Incomplete decision log tail"
    );
    let mut records = Vec::new();
    let mut previous = String::new();
    for (index, line) in bytes
        .split(|b| *b == b'\n')
        .filter(|s| !s.is_empty())
        .enumerate()
    {
        let record: Value = serde_json::from_slice(line)?;
        ensure!(
            record["seq"].as_u64() == Some(index as u64 + 1) && record["previous"] == previous,
            "Decision log sequence or chain mismatch"
        );
        let payload =
            json!({"seq":record["seq"],"previous":record["previous"],"event":record["event"]});
        let hash = digest(&payload)?;
        ensure!(record["hash"] == hash, "Decision log checksum mismatch");
        previous = hash;
        records.push(record);
    }
    Ok(records)
}

pub fn read(path: &Path) -> Result<Vec<Value>> {
    ensure!(
        fs::symlink_metadata(path)?.is_file(),
        "Decision log must be a regular file"
    );
    let mut file = fs::File::open(path)?;
    file.lock_shared()?;
    ensure!(
        file.metadata()?.len() <= LIMIT,
        "Decision log exceeds size limit"
    );
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    decode(&bytes)
}

pub fn append(path: &Path, event: Value) -> Result<String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if path.exists() {
        ensure!(
            fs::symlink_metadata(path)?.is_file(),
            "Decision log must be a regular file"
        );
    }
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    file.lock()?;
    ensure!(
        file.metadata()?.len() <= LIMIT,
        "Archive the closed decision log before continuing"
    );
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let records = decode(&bytes)?;
    if event["kind"] == "outcome" {
        let id = event["decision_id"]
            .as_str()
            .context("Decision ID required")?;
        let decision = records
            .iter()
            .find(|r| r["hash"] == id && r["event"]["kind"] == "forecast")
            .context("Unknown decision")?;
        ensure!(
            !records
                .iter()
                .any(|r| r["event"]["kind"] == "outcome" && r["event"]["decision_id"] == id),
            "Outcome already recorded"
        );
        ensure!(
            event["realized_log_return"]
                .as_f64()
                .is_some_and(f64::is_finite),
            "Finite outcome required"
        );
        ensure!(
            event["observed_at_ms"]
                .as_i64()
                .context("Outcome time required")?
                > decision["event"]["as_of_ms"]
                    .as_i64()
                    .context("Decision time missing")?,
            "Outcome predates decision input"
        );
    } else {
        ensure!(event["kind"] == "forecast", "Unknown decision event type");
    }
    let previous = records
        .last()
        .and_then(|r| r["hash"].as_str())
        .unwrap_or("");
    let mut record = json!({"seq":records.len()+1,"previous":previous,"event":event});
    let hash = digest(&record)?;
    record["hash"] = json!(hash);
    let mut encoded = serde_json::to_vec(&record)?;
    encoded.push(b'\n');
    ensure!(
        bytes.len() as u64 + encoded.len() as u64 <= LIMIT,
        "Decision log capacity exceeded"
    );
    file.seek(SeekFrom::End(0))?;
    file.write_all(&encoded)?;
    file.sync_all()?;
    Ok(hash)
}

pub fn ks(reference: &[f64], current: &[f64]) -> Result<f64> {
    ensure!(
        !reference.is_empty() && !current.is_empty(),
        "Empty drift distribution"
    );
    ensure!(
        reference.iter().chain(current).all(|v| v.is_finite()),
        "Nonfinite drift value"
    );
    let mut points: Vec<f64> = reference.iter().chain(current).copied().collect();
    points.sort_by(f64::total_cmp);
    let distance = points
        .iter()
        .map(|x| {
            (reference.iter().filter(|v| **v <= *x).count() as f64 / reference.len() as f64
                - current.iter().filter(|v| **v <= *x).count() as f64 / current.len() as f64)
                .abs()
        })
        .fold(0.0, f64::max);
    Ok(distance)
}

pub fn monitor(
    path: &Path,
    model_dir: &Path,
    window: usize,
    warn: f64,
    halt: f64,
) -> Result<Value> {
    ensure!(
        (2..=10000).contains(&window)
            && warn.is_finite()
            && halt.is_finite()
            && 0.0 <= warn
            && warn < halt
            && halt <= 1.0,
        "Invalid monitoring thresholds/window"
    );
    let model_id = quant_inference::integrity::verify_package(model_dir)?;
    let meta: Value = serde_json::from_slice(&fs::read(model_dir.join("metadata.json"))?)?;
    let reference: Vec<Vec<f64>> = serde_json::from_value(meta["monitor_reference"].clone())
        .context("Retrain model to save train-only monitor reference")?;
    let schema = digest(&meta["feature_schema"])?;
    let package_hash = format!(
        "{:x}",
        Sha256::digest(fs::read(model_dir.join("integrity.json"))?)
    );
    let records = read(path)?;
    let mut seen = BTreeSet::new();
    let decisions: Vec<_> = records
        .iter()
        .filter(|r| {
            r["event"]["kind"] == "forecast"
                && r["event"]["model"] == model_id
                && r["event"]["model_package_sha256"] == package_hash
                && r["event"]["feature_schema_sha256"] == schema
                && matches!(
                    r["event"]["data_provenance"].as_str(),
                    Some("live" | "recorded_real")
                )
        })
        .rev()
        .filter(|r| {
            seen.insert((
                r["event"]["dataset"].to_string(),
                r["event"]["as_of_ms"].to_string(),
            ))
        })
        .take(window)
        .collect();
    ensure!(!reference.is_empty(), "Empty training reference");
    let width = reference[0].len();
    ensure!(
        width > 0 && reference.iter().all(|r| r.len() == width),
        "Invalid reference shape"
    );
    let rows: Vec<Vec<f64>> = decisions
        .iter()
        .map(|r| serde_json::from_value(r["event"]["features"].clone()))
        .collect::<std::result::Result<_, _>>()?;
    ensure!(
        rows.iter().all(|r| r.len() == width),
        "Feature shape differs from training reference"
    );
    let mut drift = Vec::new();
    if !rows.is_empty() {
        for column in 0..width {
            drift.push(ks(
                &reference.iter().map(|r| r[column]).collect::<Vec<_>>(),
                &rows.iter().map(|r| r[column]).collect::<Vec<_>>(),
            )?);
        }
    }
    let selected: BTreeSet<_> = decisions
        .iter()
        .filter_map(|r| r["hash"].as_str())
        .collect();
    let mut pairs = Vec::new();
    for record in &records {
        let event = &record["event"];
        if event["kind"] == "outcome"
            && selected.contains(event["decision_id"].as_str().unwrap_or(""))
        {
            let decision = decisions
                .iter()
                .find(|d| d["hash"] == event["decision_id"])
                .unwrap();
            pairs.push((
                decision["event"]["predicted_log_return"]
                    .as_f64()
                    .context("Prediction missing")?,
                event["realized_log_return"]
                    .as_f64()
                    .context("Outcome missing")?,
            ));
        }
    }
    let ic = correlation(&pairs);
    let hit_rate = if pairs.is_empty() {
        None
    } else {
        Some(
            pairs
                .iter()
                .filter(|(x, y)| x.signum() == y.signum())
                .count() as f64
                / pairs.len() as f64,
        )
    };
    let max_ks = drift.iter().copied().fold(0.0, f64::max);
    let status = drift_status(rows.len(), window, max_ks, warn, halt);
    Ok(
        json!({"status":status,"model":model_id,"observations":rows.len(),"required_window":window,
        "outcomes":pairs.len(),"rolling_ic":ic,"hit_rate":hit_rate,"ks_by_feature":drift,"max_ks":max_ks,
        "warning":"KS uses saved train-only empirical reference; repeated requests and overlapping windows are not independent. Missing or constant outcomes yield null IC. Hash chains do not authenticate a rewritten whole log."}),
    )
}

fn drift_status(count: usize, window: usize, ks: f64, warn: f64, halt: f64) -> &'static str {
    if count < window {
        "insufficient_data"
    } else if ks >= halt {
        "halt"
    } else if ks >= warn {
        "warn"
    } else {
        "ok"
    }
}

fn correlation(pairs: &[(f64, f64)]) -> Option<f64> {
    if pairs.len() < 2 || pairs.iter().any(|(x, y)| !x.is_finite() || !y.is_finite()) {
        return None;
    }
    let n = pairs.len() as f64;
    let mx = pairs.iter().map(|p| p.0).sum::<f64>() / n;
    let my = pairs.iter().map(|p| p.1).sum::<f64>() / n;
    let cov = pairs.iter().map(|(x, y)| (x - mx) * (y - my)).sum::<f64>();
    let variance = pairs.iter().map(|(x, _)| (x - mx).powi(2)).sum::<f64>()
        * pairs.iter().map(|(_, y)| (y - my).powi(2)).sum::<f64>();
    (variance > 0.0).then(|| cov / variance.sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chain_rejects_mutation_partial_tail_and_duplicate_outcome() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("events.ndjson");
        let id = append(
            &path,
            json!({"kind":"forecast","as_of_ms":100,"predicted_log_return":0.1}),
        )
        .unwrap();
        let outcome = json!({"kind":"outcome","decision_id":id,"observed_at_ms":200,"realized_log_return":0.2});
        append(&path, outcome.clone()).unwrap();
        assert!(append(&path, outcome).is_err());
        assert_eq!(read(&path).unwrap().len(), 2);
        let original = fs::read_to_string(&path).unwrap();
        fs::write(&path, original.replace("0.2", "0.3")).unwrap();
        assert!(read(&path).is_err());
        fs::write(&path, original.trim_end()).unwrap();
        assert!(read(&path).is_err());
    }
    #[test]
    fn injected_drift_warns_and_halts_and_metrics_are_defined() {
        let baseline = vec![0., 1., 2., 3.];
        let mild = ks(&baseline, &[1., 2., 3., 4.]).unwrap();
        let severe = ks(&baseline, &[100., 101., 102., 103.]).unwrap();
        assert_eq!(drift_status(4, 4, mild, 0.2, 0.5), "warn");
        assert_eq!(drift_status(4, 4, severe, 0.2, 0.5), "halt");
        assert_eq!(drift_status(1, 4, severe, 0.2, 0.5), "insufficient_data");
        assert_eq!(correlation(&[(1., 2.), (2., 4.)]), Some(1.));
        assert_eq!(correlation(&[(1., 2.), (1., 4.)]), None);
    }
}
