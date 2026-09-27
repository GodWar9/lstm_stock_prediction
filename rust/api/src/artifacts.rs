//! Immutable run exports; the manifest is published last, after all artifacts exist.
use crate::contract::{Instrument, Provenance, RunManifest};
use anyhow::{ensure, Result};
use arrow2::{
    array::{Array, Float64Array},
    chunk::Chunk,
    datatypes::{DataType, Field, Schema},
    io::ipc::write::{FileWriter, WriteOptions},
};
use quant_backtest::BacktestReport;
use std::{collections::BTreeMap, fs, path::Path};

pub fn arrow_bytes(columns: &[(&str, Vec<f64>)]) -> Result<Vec<u8>> {
    let n = columns.first().map_or(0, |(_, v)| v.len());
    ensure!(
        columns.iter().all(|(_, v)| v.len() == n),
        "Unaligned series"
    );
    let schema = Schema::from(
        columns
            .iter()
            .map(|(name, _)| Field::new(*name, DataType::Float64, false))
            .collect::<Vec<_>>(),
    );
    let arrays: Vec<Box<dyn Array>> = columns
        .iter()
        .map(|(_, v)| Float64Array::from_slice(v).boxed())
        .collect();
    let mut writer =
        FileWriter::try_new(Vec::new(), schema, None, WriteOptions { compression: None })?;
    writer.write(&Chunk::new(arrays), None)?;
    writer.finish()?;
    Ok(writer.into_inner())
}

pub fn publish_backtest(
    root: &Path,
    report: &BacktestReport,
    provenance: Provenance,
    symbol: &str,
    split: &str,
) -> Result<RunManifest> {
    let mut manifest = RunManifest {
        schema_version: 1, run_id: uuid::Uuid::new_v4().to_string(), created_at: chrono::Utc::now().to_rfc3339(),
        kind: "backtest".into(), split: split.into(), provenance,
        instruments: vec![Instrument { id: symbol.into(), kind: "equity".into() }],
        capabilities: vec!["equity_curve".into(), "drawdown".into(), "trades".into()],
        artifacts: BTreeMap::new(),
        metrics: BTreeMap::from([("sharpe".into(), report.sharpe), ("sortino".into(), report.sortino),
            ("max_drawdown".into(), report.max_drawdown), ("total_return".into(), report.total_return_pct),
            ("turnover".into(), report.turnover), ("hit_rate".into(), report.hit_rate), ("final_nav".into(), report.final_nav)]),
        warnings: vec!["Daily-bar annualization uses 252 periods per year. Deflated Sharpe is a heuristic, not a significance test.".into()],
    };
    if manifest.provenance.source == "synthetic" {
        manifest.warnings.push(
            "Synthetic market data: these results do not demonstrate trading performance.".into(),
        );
    }
    let dir = root.join(&manifest.run_id);
    fs::create_dir_all(&dir)?;
    let mut peak = report.initial_cash;
    let drawdown = report
        .equity_curve
        .iter()
        .map(|(_, nav)| {
            peak = peak.max(*nav);
            if peak > 0.0 {
                (peak - nav) / peak
            } else {
                0.0
            }
        })
        .collect();
    fs::write(
        dir.join("equity.arrow"),
        arrow_bytes(&[
            (
                "timestamp_ms",
                report
                    .equity_curve
                    .iter()
                    .map(|(ts, _)| (*ts / 1_000_000) as f64)
                    .collect(),
            ),
            (
                "nav",
                report.equity_curve.iter().map(|(_, nav)| *nav).collect(),
            ),
            ("drawdown", drawdown),
        ])?,
    )?;
    fs::write(
        dir.join("trades.arrow"),
        arrow_bytes(&[
            (
                "timestamp_ms",
                report
                    .trade_log
                    .iter()
                    .map(|f| (f.as_of / 1_000_000) as f64)
                    .collect(),
            ),
            (
                "fill_price",
                report.trade_log.iter().map(|f| f.fill_price).collect(),
            ),
            (
                "quantity",
                report.trade_log.iter().map(|f| f.fill_quantity).collect(),
            ),
            (
                "commission",
                report.trade_log.iter().map(|f| f.commission).collect(),
            ),
            (
                "slippage",
                report.trade_log.iter().map(|f| f.slippage).collect(),
            ),
        ])?,
    )?;
    fs::write(dir.join("report.json"), serde_json::to_vec_pretty(report)?)?;
    manifest
        .artifacts
        .insert("equity".into(), "equity.arrow".into());
    manifest
        .artifacts
        .insert("trades".into(), "trades.arrow".into());
    manifest
        .artifacts
        .insert("report".into(), "report.json".into());

    if !report.benchmark_curve.is_empty() {
        let mut b_peak = report.initial_cash;
        let b_drawdown: Vec<f64> = report
            .benchmark_curve
            .iter()
            .map(|(_, nav)| {
                b_peak = b_peak.max(*nav);
                if b_peak > 0.0 {
                    (b_peak - nav) / b_peak
                } else {
                    0.0
                }
            })
            .collect();
        fs::write(
            dir.join("benchmark.arrow"),
            arrow_bytes(&[
                (
                    "timestamp_ms",
                    report
                        .benchmark_curve
                        .iter()
                        .map(|(ts, _)| (*ts / 1_000_000) as f64)
                        .collect(),
                ),
                (
                    "nav",
                    report.benchmark_curve.iter().map(|(_, nav)| *nav).collect(),
                ),
                ("drawdown", b_drawdown),
            ])?,
        )?;
        manifest.capabilities.push("benchmark".into());
        manifest
            .artifacts
            .insert("benchmark".into(), "benchmark.arrow".into());
        manifest.metrics.insert(
            "benchmark_total_return".into(),
            report.benchmark_total_return,
        );
    }

    if !report.positions_curve.is_empty() {
        fs::write(
            dir.join("positions.arrow"),
            arrow_bytes(&[
                (
                    "timestamp_ms",
                    report
                        .positions_curve
                        .iter()
                        .map(|(ts, _, _)| (*ts / 1_000_000) as f64)
                        .collect(),
                ),
                (
                    "quantity",
                    report.positions_curve.iter().map(|(_, q, _)| *q).collect(),
                ),
                (
                    "market_value",
                    report.positions_curve.iter().map(|(_, _, v)| *v).collect(),
                ),
            ])?,
        )?;
        manifest.capabilities.push("positions".into());
        manifest
            .artifacts
            .insert("positions".into(), "positions.arrow".into());
    }

    Ok(manifest)
}

pub fn write_manifest(dir: &Path, manifest: &RunManifest) -> Result<()> {
    fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec_pretty(manifest)?,
    )?;
    Ok(())
}
