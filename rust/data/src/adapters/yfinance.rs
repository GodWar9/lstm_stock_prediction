//! YfinanceAdapter: Free/experimental market data feed via Python subprocess helper.

use crate::provider::{DataError, MarketDataProvider};
use crate::types::{Bar, CorporateAction, Timestamp};
use crate::validate::{check_duplicate_timestamps, validate_bars_monotonic_and_sound};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use quant_calendar::{TradingCalendar, UsEquityCalendar};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Free/experimental market data adapter powered by yfinance via an isolated Python child process.
#[derive(Debug, Clone)]
pub struct YfinanceAdapter {
    python_bin: PathBuf,
    helper_script: PathBuf,
    cache_dir: PathBuf,
}

impl YfinanceAdapter {
    pub fn new(
        python_bin: impl Into<PathBuf>,
        helper_script: impl Into<PathBuf>,
        cache_dir: impl Into<PathBuf>,
    ) -> Self {
        let cache_dir = cache_dir.into();
        let _ = fs::create_dir_all(&cache_dir);
        Self {
            python_bin: python_bin.into(),
            helper_script: helper_script.into(),
            cache_dir,
        }
    }

    /// Automatically discovers the default python executable and helper script path relative to workspace.
    pub fn default_paths() -> Self {
        let root = Path::new(".");
        let local = if cfg!(windows) {
            "python/.venv/Scripts/python.exe"
        } else {
            "python/.venv/bin/python"
        };
        let python = std::env::var("QUANTCTL_PYTHON")
            .or_else(|_| std::env::var("PYTHON_BIN"))
            .unwrap_or_else(|_| {
                if Path::new(local).is_file() {
                    local.into()
                } else {
                    "python".into()
                }
            });
        let script = root.join("python/ml/data_fetch_helper.py");
        let cache = root.join("datasets/cache/yfinance");
        Self::new(python, script, cache)
    }

    fn run_helper(&self, args: &[&str]) -> Result<String, DataError> {
        if std::env::var("QUANTCTL_ALLOW_NETWORK").as_deref() != Ok("1") {
            return Err(DataError::FetchError("Network acquisition is disabled. Use provider: csv for offline data, or explicitly set QUANTCTL_ALLOW_NETWORK=1 on a connected acquisition machine".into()));
        }
        let output = Command::new(&self.python_bin)
            .arg(&self.helper_script)
            .args(args)
            .output()
            .map_err(|e| DataError::FetchError(format!("Failed to spawn python helper: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DataError::FetchError(format!(
                "Python yfinance fetcher failed (status {:?}): {}",
                output.status, stderr
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

impl MarketDataProvider for YfinanceAdapter {
    fn fetch_ohlcv(
        &self,
        symbol: &str,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<Bar>, DataError> {
        crate::validate_storage_id(symbol)?;
        let output_file = self
            .cache_dir
            .join(format!("{}_{}_{}.csv", symbol, start, end));
        let output_str = output_file.to_string_lossy();

        let _ = self.run_helper(&[
            "--symbol",
            symbol,
            "--start",
            &start.to_string(),
            "--end",
            &end.to_string(),
            "--output",
            &output_str,
        ])?;

        let content = fs::read_to_string(&output_file)
            .map_err(|e| DataError::FetchError(format!("Failed to read downloaded CSV: {}", e)))?;

        let mut bars = Vec::new();
        for (line_idx, line) in content.lines().enumerate() {
            if line_idx == 0 || line.trim().is_empty() {
                // Header or empty line
                continue;
            }

            let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
            if parts.len() != 6 {
                return Err(DataError::ParseError(format!(
                    "Malformed OHLCV row {}",
                    line_idx + 1
                )));
            }

            let dt_str = parts[0];
            let dt: DateTime<Utc> = if let Ok(dt) = DateTime::parse_from_rfc3339(dt_str) {
                dt.with_timezone(&Utc)
            } else if let Ok(nd) = NaiveDate::parse_from_str(dt_str, "%Y-%m-%d") {
                Utc.from_utc_datetime(&nd.and_hms_opt(21, 0, 0).unwrap())
            } else {
                return Err(DataError::ParseError(format!(
                    "Invalid date format in line: {}",
                    line
                )));
            };

            let open: f64 = parts[1]
                .parse()
                .map_err(|_| DataError::ParseError("Invalid open".into()))?;
            let high: f64 = parts[2]
                .parse()
                .map_err(|_| DataError::ParseError("Invalid high".into()))?;
            let low: f64 = parts[3]
                .parse()
                .map_err(|_| DataError::ParseError("Invalid low".into()))?;
            let close: f64 = parts[4]
                .parse()
                .map_err(|_| DataError::ParseError("Invalid close".into()))?;
            let volume: u64 = parts[5]
                .parse()
                .map_err(|_| DataError::ParseError("Invalid volume".into()))?;

            let ts = Timestamp::from_datetime(dt);
            bars.push(Bar::same_bar(ts, open, high, low, close, volume));
        }

        // Validate data integrity and point-in-time constraints
        validate_bars_monotonic_and_sound(&bars)?;
        check_duplicate_timestamps(&bars)?;

        Ok(bars)
    }

    fn corporate_actions(
        &self,
        symbol: &str,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<CorporateAction>, DataError> {
        crate::validate_storage_id(symbol)?;
        let output_file = self
            .cache_dir
            .join(format!("{}_actions_{}_{}.csv", symbol, start, end));
        let output_str = output_file.to_string_lossy();

        let _ = self.run_helper(&[
            "--symbol",
            symbol,
            "--actions",
            "--start",
            &start.to_string(),
            "--end",
            &end.to_string(),
            "--output",
            &output_str,
        ])?;

        let mut actions = Vec::new();
        if let Ok(content) = fs::read_to_string(&output_file) {
            for (line_idx, line) in content.lines().enumerate() {
                if line_idx == 0 || line.trim().is_empty() {
                    continue;
                }
                let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
                if parts.len() < 3 {
                    continue;
                }
                if let Ok(date) = NaiveDate::parse_from_str(parts[0], "%Y-%m-%d") {
                    let action_type = parts[1];
                    let val: f64 = parts[2].parse().unwrap_or(0.0);
                    if action_type == "SPLIT" {
                        actions.push(CorporateAction::split(symbol, date, val));
                    } else if action_type == "DIVIDEND" {
                        actions.push(CorporateAction::dividend(symbol, date, val));
                    }
                }
            }
        }

        Ok(actions)
    }

    fn trading_calendar(&self, exchange: &str) -> Result<Box<dyn TradingCalendar>, DataError> {
        Ok(Box::new(UsEquityCalendar::new(exchange)))
    }
}
