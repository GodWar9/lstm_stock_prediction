//! On-demand inference over an immutable, integrity-checked market dataset.
use anyhow::{bail, ensure, Context, Result};
use quant_data::{Bar, DatasetManifest};
use quant_inference::{provider::PredictionProvider, OnnxLstmProvider};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize, IntoParams)]
#[serde(deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct ForecastQuery {
    #[serde(default)]
    #[param(required = false)]
    pub fresh: bool,
    #[serde(default)]
    #[param(required = false)]
    pub live: bool,
    pub model: String,
    pub dataset: String,
    pub symbol: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DatasetChoice {
    pub market: Option<serde_json::Value>,
    pub dataset: String,
    pub symbol: String,
    pub interval: String,
    pub source: String,
    pub bars: usize,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Forecast {
    pub data_provenance: DataProvenance,
    pub market: Option<serde_json::Value>,
    pub input_mode: String,
    pub fetched_at_ms: Option<i64>,
    pub data_age_ms: i64,
    pub feed_latency: String,
    pub model: String,
    pub dataset: String,
    pub symbol: String,
    pub interval: String,
    pub source: String,
    pub as_of_ms: i64,
    pub available_at_ms: i64,
    pub generated_at_ms: i64,
    pub horizon_bars: usize,
    pub last_close: f64,
    pub predicted_log_return: f64,
    pub predicted_return: f64,
    pub implied_close: f64,
    pub dataset_sha256: String,
    pub model_package_sha256: String,
    pub warnings: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DataProvenance {
    Synthetic,
    RecordedReal,
    Live,
}

fn data_provenance(dataset: &str, model: Option<&str>, live: bool) -> Result<DataProvenance> {
    let synthetic = |source: &str| {
        let source = source.to_ascii_lowercase();
        ["synthetic", "fixture", "mock"]
            .iter()
            .any(|tag| source.contains(tag))
    };
    // Model training provenance must never disappear when choosing another dataset.
    if synthetic(dataset) || model.is_some_and(synthetic) {
        return Ok(DataProvenance::Synthetic);
    }
    ensure!(
        !dataset.is_empty()
            && dataset != "unknown"
            && model.is_some_and(|source| !source.is_empty() && source != "unknown"),
        "Model and dataset source provenance is required; retrain legacy artifacts"
    );
    Ok(if live {
        DataProvenance::Live
    } else {
        DataProvenance::RecordedReal
    })
}

fn directory(root: &Path, id: &str) -> Result<PathBuf> {
    quant_data::validate_storage_id(id)?;
    let base = root
        .canonicalize()
        .context("Artifact root is unavailable")?;
    let path = base
        .join(id)
        .canonicalize()
        .context("Artifact ID is unavailable")?;
    ensure!(
        path.starts_with(base) && path.is_dir(),
        "Artifact escapes its root"
    );
    Ok(path)
}

fn read(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let meta = fs::symlink_metadata(path).context("Required artifact file is unavailable")?;
    ensure!(
        meta.is_file() && meta.len() <= limit,
        "Artifact file type or size is unsupported"
    );
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= limit, "Artifact exceeds size limit");
    Ok(bytes)
}

pub fn datasets(root: &Path) -> Result<Vec<DatasetChoice>> {
    let base = root.join("datasets/market");
    if !base.exists() {
        return Ok(Vec::new());
    }
    let mut choices = Vec::new();
    for (scanned, entry) in fs::read_dir(&base)?.take(1001).enumerate() {
        ensure!(
            scanned < 1000,
            "Archive older datasets before listing more than 1000 versions"
        );
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().to_string();
        let dir = directory(&base, &id)?;
        for (count, file) in fs::read_dir(dir)?.take(1001).enumerate() {
            ensure!(count < 1000, "Dataset directory exceeds 1000 entries");
            let file = file?;
            if !file
                .file_name()
                .to_string_lossy()
                .ends_with(".manifest.json")
            {
                continue;
            }
            ensure!(choices.len() < 1000, "Dataset listing exceeds 1000 symbols");
            let manifest: DatasetManifest = serde_json::from_slice(&read(&file.path(), 65536)?)?;
            quant_data::validate_storage_id(&manifest.symbol)?;
            ensure!(
                manifest.dataset_version == id,
                "Dataset manifest identity mismatch"
            );
            choices.push(DatasetChoice {
                market: manifest
                    .market
                    .as_ref()
                    .map(serde_json::to_value)
                    .transpose()?,
                dataset: id.clone(),
                symbol: manifest.symbol,
                interval: manifest.bar_interval,
                source: manifest.source,
                bars: manifest.bar_count,
            });
        }
    }
    choices.sort_by(|a, b| (&a.dataset, &a.symbol).cmp(&(&b.dataset, &b.symbol)));
    Ok(choices)
}

pub fn run(root: &Path, q: &ForecastQuery) -> Result<Forecast> {
    run_with_market(root, q, &crate::market::MarketService::default())
}

pub(crate) fn run_with_market(
    root: &Path,
    q: &ForecastQuery,
    market_service: &crate::market::MarketService,
) -> Result<Forecast> {
    run_with_market_and_live(root, q, market_service, &crate::live::disabled())
}

pub(crate) fn run_with_market_and_live(
    root: &Path,
    q: &ForecastQuery,
    market_service: &crate::market::MarketService,
    live: &crate::live::Live,
) -> Result<Forecast> {
    ensure!(
        !(q.live && q.fresh),
        "Choose either live or fresh input mode"
    );
    quant_data::validate_storage_id(&q.symbol)?;
    let model = directory(&root.join("models"), &q.model)?;
    let dataset = directory(&root.join("datasets/market"), &q.dataset)?;
    // Bound all package files before ONNX allocation; the provider verifies their hashes.
    for name in [
        "integrity.json",
        "metadata.json",
        "scaler.json",
        "validation.json",
        "training_log.json",
    ] {
        read(&model.join(name), 8 * 1024 * 1024)?;
    }
    for name in ["model.onnx", "model.pt"] {
        let m = fs::symlink_metadata(model.join(name))?;
        ensure!(
            m.is_file() && m.len() <= 64 * 1024 * 1024,
            "Model exceeds inference size limit"
        );
    }
    let meta: serde_json::Value =
        serde_json::from_slice(&read(&model.join("metadata.json"), 8 * 1024 * 1024)?)?;
    let mut manifest: DatasetManifest = serde_json::from_slice(&read(
        &dataset.join(format!("{}.manifest.json", q.symbol)),
        65536,
    )?)?;
    ensure!(
        manifest.dataset_version == q.dataset && manifest.symbol == q.symbol,
        "Dataset identity mismatch"
    );
    let interval = meta["bar_interval"].as_str().unwrap_or("1d");
    ensure!(
        interval == manifest.bar_interval,
        "Model and dataset bar intervals differ; train an interval-matched model"
    );
    let mut periods = match interval {
        "1d" => 252.0,
        "1m" => 252.0 * 390.0,
        _ => bail!("Unsupported bar interval"),
    };
    let bytes = read(
        &dataset.join(format!("{}.json", q.symbol)),
        64 * 1024 * 1024,
    )?;
    ensure!(
        format!("{:x}", Sha256::digest(&bytes)) == manifest.content_sha256,
        "Market dataset integrity mismatch"
    );
    let mut bars: Vec<Bar> = serde_json::from_slice(&bytes)?;
    ensure!(
        !bars.is_empty() && bars.len() == manifest.bar_count && bars.len() <= 100_000,
        "Dataset row count is unsupported"
    );
    quant_data::validate_bars_monotonic_and_sound(&bars)?;
    let now = chrono::Utc::now().timestamp_millis();
    ensure!(
        bars.iter().all(|b| b.availability_timestamp >= b.timestamp
            && b.availability_timestamp.as_nanos() / 1_000_000 <= now)
            && bars
                .windows(2)
                .all(|b| b[1].availability_timestamp > b[0].availability_timestamp),
        "Invalid or future bar availability"
    );
    let provider = OnnxLstmProvider::load(&model)?;
    let metadata = provider.metadata();
    ensure!(
        provider.model_id() == q.model && metadata.feature_set_version == 1,
        "Model identity or feature version is unsupported"
    );
    ensure!(
        metadata.target_definition.transformation == "log_return"
            && metadata.target_definition.price_field == "close"
            && metadata.target_definition.target_type == "regression"
            && metadata.target_definition.horizon > 0,
        "Unsupported forecast target"
    );
    let mut fetched_at_ms = None;
    let identity = manifest
        .market
        .as_ref()
        .map(serde_json::to_value)
        .transpose()?;
    if identity.is_some() || !meta["market"].is_null() {
        ensure!(
            identity.as_ref() == Some(&meta["market"]),
            "Model and dataset market identities differ; train a matching model"
        );
        let market = manifest
            .market
            .as_ref()
            .context("Dataset market identity missing")?;
        market.validate().map_err(anyhow::Error::msg)?;
        periods = market.periods_per_year;
        ensure!(
            meta["periods_per_year"].as_f64() == Some(periods),
            "Model feature annualization differs from the market"
        );
    }
    let mut feed_latency_str = "unknown".to_string();
    if q.live {
        let live_state = live.blocking_read();
        let source = format!("alpaca:{}:first_publication", live_state.feed);
        ensure!(
            interval == "1m",
            "Live forecasts require a 1m model and dataset"
        );
        ensure!(
            manifest.source == source
                && meta["source"].as_str() == Some(source.as_str())
                && meta["training_symbol"].as_str() == Some(q.symbol.as_str()),
            "Live forecasts require a model and dataset trained on the same symbol and Alpaca feed"
        );
        bars = live_inputs(&live_state, &q.symbol, bars, now)?;
        let last_bar = bars.last().unwrap();
        let latency = (last_bar.availability_timestamp.as_nanos() / 1_000_000)
            .saturating_sub(last_bar.timestamp.as_nanos() / 1_000_000);
        feed_latency_str = format!("{latency} ms");
        manifest.content_sha256 =
            format!("{:x}", Sha256::digest(serde_json::to_vec_pretty(&bars)?));
    } else if q.fresh {
        ensure!(meta["source"].as_str() == Some("twelve_data:unadjusted:rest_snapshot")
            && manifest.source == "twelve_data:unadjusted:rest_snapshot"
            && meta["training_symbol"].as_str() == Some(q.symbol.as_str()),
            "Fresh forecasts require a model trained on matching Twelve Data history; demo and legacy models cannot use fresh inputs");
        let market = manifest
            .market
            .as_ref()
            .context("Train a global market model before requesting fresh data")?;
        let snapshot = market_service.snapshot(market, interval)?;
        crate::market::require_recent(&snapshot, interval, chrono::Utc::now().timestamp_millis())?;
        fetched_at_ms = Some(snapshot.fetched_at_ms);
        bars = snapshot.bars;
        manifest.content_sha256 =
            format!("{:x}", Sha256::digest(serde_json::to_vec_pretty(&bars)?));
    }
    let rows = quant_features::standard_graph(periods).compute_batch(&bars);
    ensure!(
        rows.len() >= provider.lookback(),
        "Insufficient bars for feature warmup and model lookback"
    );
    let last = bars.last().unwrap();
    ensure!(
        rows.last().unwrap().timestamp == last.availability_timestamp,
        "Latest bar did not produce a complete feature row"
    );
    let mut input = Vec::new();
    for row in &rows[rows.len() - provider.lookback()..] {
        for name in provider.feature_schema() {
            let value = *row.values.get(name).context(
                "Model feature is unavailable; retrain on the current Rust feature schema",
            )?;
            ensure!(value.is_finite(), "Non-finite model input");
            input.push(value);
        }
    }
    let prediction =
        provider.predict(&input, provider.lookback(), provider.feature_schema().len())?;
    let predicted_return = prediction.value.exp_m1();
    let implied_close = last.close * prediction.value.exp();
    ensure!(
        prediction.value.is_finite()
            && predicted_return.is_finite()
            && implied_close.is_finite()
            && implied_close > 0.0,
        "Invalid model output"
    );
    let mut warnings = vec![
        if q.live {
            "Live price stream forecast; feature window updated from Alpaca WebSocket feed with strict point-in-time guarantees.".into()
        } else if q.fresh {
            "Fresh REST snapshot forecast; feed delay and exchange entitlement are not independently verified.".into()
        } else {
            "Recorded dataset forecast; this request does not consume the live price stream.".into()
        },
        "Implied close is a model estimate, not a calibrated price interval or probability.".into(),
    ];
    if q.live {
        warnings
            .push("Point-in-time invariant: live bars available only after complete close.".into());
    }
    let provenance = data_provenance(&manifest.source, meta["source"].as_str(), q.live)?;
    if provenance == DataProvenance::Synthetic {
        warnings.push("Synthetic or fixture data: use only to verify the workflow.".into());
    }
    let age = now - last.timestamp.as_nanos() / 1_000_000;
    if !q.fresh
        && !q.live
        && age
            > if interval == "1m" {
                120_000
            } else {
                4 * 86_400_000
            }
    {
        warnings.push("Historical data: the forecast is relative to the displayed bar, not the current market.".into());
    }
    warnings.push("Training-symbol compatibility and out-of-sample status are not certified for this selection.".into());
    if identity.is_some() {
        warnings.push("Historical REST bars may be revised; close-time availability is estimated, not a first-publication record. Corporate-action adjustments are disabled.".into());
        warnings.push("Global exchange holidays and missing session bars are not independently validated. Horizon is measured in observed bars.".into());
    }
    let now = chrono::Utc::now().timestamp_millis();
    let result = Forecast {
        data_provenance: provenance,
        market: identity,
        input_mode: if q.live {
            "live"
        } else if q.fresh {
            "fresh"
        } else {
            "recorded"
        }
        .into(),
        fetched_at_ms,
        data_age_ms: now - last.timestamp.as_nanos() / 1_000_000,
        feed_latency: feed_latency_str,
        model: q.model.clone(),
        dataset: q.dataset.clone(),
        symbol: q.symbol.clone(),
        interval: interval.into(),
        source: if q.live {
            "alpaca:live_stream".into()
        } else {
            manifest.source
        },
        as_of_ms: last.timestamp.as_nanos() / 1_000_000,
        available_at_ms: last.availability_timestamp.as_nanos() / 1_000_000,
        generated_at_ms: now,
        horizon_bars: metadata.target_definition.horizon,
        last_close: last.close,
        predicted_log_return: prediction.value,
        predicted_return,
        implied_close,
        dataset_sha256: manifest.content_sha256,
        model_package_sha256: format!(
            "{:x}",
            Sha256::digest(read(&model.join("integrity.json"), 8 * 1024 * 1024)?)
        ),
        warnings,
    };
    if q.live {
        let mut state = live.blocking_write();
        // An inference that raced a reconnect or another minute is not current.
        if state.status == "connected"
            && state
                .bar_buffers
                .get(&q.symbol)
                .and_then(|bars| bars.last())
                .is_some_and(|bar| {
                    bar.timestamp.as_nanos() / 1_000_000 == result.as_of_ms
                        && bar.availability_timestamp.as_nanos() / 1_000_000
                            == result.available_at_ms
                })
        {
            state.live_forecasts.insert(
                q.symbol.clone(),
                crate::live::LiveForecast {
                    data_provenance: result.data_provenance,
                    model: q.model.clone(),
                    dataset: q.dataset.clone(),
                    symbol: q.symbol.clone(),
                    as_of_ms: result.as_of_ms,
                    available_at_ms: result.available_at_ms,
                    generated_at_ms: result.generated_at_ms,
                    data_age_ms: result.data_age_ms,
                    feed_latency: result.feed_latency.clone(),
                    last_close: result.last_close,
                    predicted_log_return: result.predicted_log_return,
                    predicted_return: result.predicted_return,
                    implied_close: result.implied_close,
                    bars_in_window: bars.len(),
                    status: "ready".into(),
                    warnings: result.warnings.clone(),
                },
            );
        }
    }
    Ok(result)
}

fn live_inputs(
    state: &crate::live::Snapshot,
    symbol: &str,
    history: Vec<Bar>,
    now: i64,
) -> Result<Vec<Bar>> {
    ensure!(
        state.status == "connected",
        "Live feed is not connected; wait for ingestion to recover"
    );
    ensure!(
        matches!(state.feed.as_str(), "iex" | "sip"),
        "Test and delayed feeds cannot produce live forecasts"
    );
    ensure!(
        state.symbols.iter().any(|s| s == symbol),
        "Symbol is not subscribed to the live feed"
    );
    let live = state
        .bar_buffers
        .get(symbol)
        .filter(|bars| !bars.is_empty())
        .context("Waiting for the first completed regular-session live minute bar")?;
    let last = live.last().unwrap();
    ensure!(
        (0..=120_000).contains(&(now - last.timestamp.as_nanos() / 1_000_000))
            && (0..=120_000).contains(&(now - last.availability_timestamp.as_nanos() / 1_000_000)),
        "Live minute bars are stale or future-dated; wait for a new completed bar"
    );
    let first = &live[0];
    let mut bars: Vec<_> = history
        .into_iter()
        .filter(|b| {
            b.timestamp < first.timestamp && b.availability_timestamp < first.availability_timestamp
        })
        .collect();
    // Old history cannot conceal a capture gap.
    if bars.last().is_some_and(|b| {
        !quant_data::capture::consecutive_minutes(b.timestamp, first.timestamp)
            || b.availability_timestamp >= first.timestamp
    }) {
        bars.clear();
    }
    bars.extend_from_slice(live);
    quant_data::validate_bars_monotonic_and_sound(&bars)?;
    ensure!(
        bars.iter()
            .all(|b| quant_data::capture::regular_minute(b.timestamp)
                && b.availability_timestamp >= b.timestamp
                && b.availability_timestamp.as_nanos() / 1_000_000 <= now),
        "Invalid live minute availability or session"
    );
    ensure!(
        bars.windows(2)
            .all(
                |p| quant_data::capture::consecutive_minutes(p[0].timestamp, p[1].timestamp)
                    && p[0].availability_timestamp < p[1].timestamp
            ),
        "Missing or late minute bars in live feature window"
    );
    Ok(bars)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn synthetic_training_provenance_cannot_be_hidden_by_real_inputs() {
        for source in ["synthetic", "fixture", "mock", "Synthetic_demo"] {
            for live in [false, true] {
                let provenance = data_provenance("csv", Some(source), live).unwrap();
                assert_eq!(serde_json::to_value(provenance).unwrap(), "synthetic");
                assert_eq!(
                    data_provenance(source, Some("csv"), live).unwrap(),
                    provenance
                );
            }
        }
        assert!(data_provenance("csv", None, false).is_err());
        assert!(data_provenance("unknown", Some("csv"), false).is_err());
        assert_eq!(
            data_provenance("csv", Some("csv"), false).unwrap(),
            DataProvenance::RecordedReal
        );
        assert_eq!(
            data_provenance(
                "alpaca:iex:first_publication",
                Some("alpaca:iex:first_publication"),
                true
            )
            .unwrap(),
            DataProvenance::Live
        );
    }
    #[test]
    fn rejects_paths_and_handles_empty_workspace() {
        let root = tempfile::tempdir().unwrap();
        assert!(datasets(root.path()).unwrap().is_empty());
        assert!(run(
            root.path(),
            &ForecastQuery {
                fresh: false,
                live: false,
                model: "../outside".into(),
                dataset: "x".into(),
                symbol: "AAPL".into()
            }
        )
        .is_err());
        assert!(run(
            root.path(),
            &ForecastQuery {
                fresh: false,
                live: false,
                model: "x".into(),
                dataset: "x".into(),
                symbol: "../secret".into()
            }
        )
        .is_err());
    }

    fn minute(time: &str) -> Bar {
        let ns = chrono::DateTime::parse_from_rfc3339(time)
            .unwrap()
            .timestamp_nanos_opt()
            .unwrap();
        Bar::new(
            quant_data::Timestamp(ns),
            quant_data::Timestamp(ns + 500_000_000),
            100.0,
            101.0,
            99.0,
            100.0,
            10,
            false,
        )
    }

    #[test]
    fn live_inputs_require_connected_fresh_real_bars_and_continuity() {
        let live = crate::live::disabled();
        let mut state = live.blocking_write();
        state.symbols = vec!["AAPL".into()];
        let bar = minute("2026-09-28T14:01:00Z");
        let now = bar.availability_timestamp.as_nanos() / 1_000_000;
        assert!(live_inputs(&state, "AAPL", vec![bar.clone()], now).is_err());
        state.status = "connected".into();
        assert!(live_inputs(&state, "AAPL", vec![bar.clone()], now)
            .unwrap_err()
            .to_string()
            .contains("first completed"));
        state.bar_buffers.insert("AAPL".into(), vec![bar.clone()]);
        let history = vec![minute("2026-09-28T14:00:00Z")];
        assert_eq!(live_inputs(&state, "AAPL", history, now).unwrap().len(), 2);
        // Distant history is discarded, so feature warmup must come from new bars.
        assert_eq!(
            live_inputs(&state, "AAPL", vec![minute("2026-09-28T13:59:00Z")], now)
                .unwrap()
                .len(),
            1
        );
        assert!(live_inputs(&state, "AAPL", vec![], now + 120_001).is_err());
        assert!(live_inputs(&state, "AAPL", vec![], now - 1000).is_err());
        assert!(live_inputs(&state, "MSFT", vec![], now).is_err());
        state.feed = "test".into();
        assert!(live_inputs(&state, "AAPL", vec![], now).is_err());
        state.feed = "iex".into();
        state
            .bar_buffers
            .get_mut("AAPL")
            .unwrap()
            .insert(0, minute("2026-09-28T13:59:00Z"));
        assert!(live_inputs(&state, "AAPL", vec![], now).is_err());
    }

    #[test]
    fn live_forecast_rejects_when_live_stream_disabled() {
        let root = tempfile::tempdir().unwrap();
        let res = run(
            root.path(),
            &ForecastQuery {
                fresh: false,
                live: true,
                model: "inspector_demo".into(),
                dataset: "x".into(),
                symbol: "AAPL".into(),
            },
        );
        assert!(res.is_err());
    }
}
