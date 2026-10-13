//! Shared provider budget and short-lived immutable snapshots for fresh forecasts.
use anyhow::{ensure, Context, Result};
use quant_data::{global::Snapshot, MarketIdentity};
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct MarketService {
    state: Mutex<State>,
}
#[derive(Default)]
struct State {
    last_request: Option<Instant>,
    day: i64,
    requests: usize,
    cache: HashMap<String, (Instant, Snapshot)>,
}
impl MarketService {
    pub fn snapshot(&self, market: &MarketIdentity, interval: &str) -> Result<Snapshot> {
        ensure!(
            quant_data::global::key_configured(),
            "Set TWELVE_DATA_API_KEY on the backend to enable fresh global market data"
        );
        self.load(market, interval, || {
            quant_data::global::fetch(market, interval, None, 5000)
        })
    }
    fn load(
        &self,
        market: &MarketIdentity,
        interval: &str,
        fetch: impl FnOnce() -> Result<Snapshot>,
    ) -> Result<Snapshot> {
        let key = format!("{}:{interval}", serde_json::to_string(market)?);
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Market cache unavailable"))?;
        if let Some((at, snapshot)) = state.cache.get(&key) {
            if at.elapsed() < Duration::from_secs(120) {
                return Ok(snapshot.clone());
            }
        }
        ensure!(
            state
                .last_request
                .is_none_or(|at| at.elapsed() >= Duration::from_secs(60)),
            "Market request budget is cooling down; retry after 60 seconds"
        );
        let day = chrono::Utc::now().timestamp().div_euclid(86400);
        if state.day != day {
            state.day = day;
            state.requests = 0;
        }
        ensure!(
            state.requests < 300,
            "Daily backend market request budget reached; retry tomorrow UTC"
        );
        state.last_request = Some(Instant::now());
        state.requests += 1;
        let snapshot = fetch()?;
        if state.cache.len() >= 64 {
            state.cache.clear();
        }
        state.cache.insert(key, (Instant::now(), snapshot.clone()));
        Ok(snapshot)
    }
}

/// Freshness is independent from exchange entitlements and is checked again on cache hits.
pub fn require_recent(snapshot: &Snapshot, interval: &str, now_ms: i64) -> Result<()> {
    let latest = snapshot
        .bars
        .last()
        .context("No completed market bars")?
        .timestamp
        .as_nanos()
        / 1_000_000;
    let limit = if interval == "1m" {
        180_000
    } else {
        4 * 86_400_000
    };
    ensure!(latest <= now_ms && now_ms - latest <= limit,
        "Latest completed market bar is stale or the market is closed; use a recorded forecast or retry during market hours");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use quant_data::{Bar, Timestamp};
    fn market() -> MarketIdentity {
        MarketIdentity {
            provider_symbol: "AAPL".into(),
            exchange: "NASDAQ".into(),
            currency: "USD".into(),
            timezone: "America/New_York".into(),
            instrument_type: "Common Stock".into(),
            periods_per_year: 252.0,
        }
    }
    #[test]
    fn coalesces_requests_and_throttles_other_symbols_and_failures() {
        let service = MarketService::default();
        let snapshot = Snapshot {
            bars: vec![],
            fetched_at_ms: 1,
        };
        service.load(&market(), "1d", || Ok(snapshot)).unwrap();
        service
            .load(&market(), "1d", || panic!("cache must be reused"))
            .unwrap();
        assert!(service
            .load(&market(), "1m", || panic!("global cooldown"))
            .is_err());
        let failed = MarketService::default();
        assert!(failed
            .load(&market(), "1d", || anyhow::bail!("provider rejected"))
            .is_err());
        assert!(failed
            .load(&market(), "1d", || panic!("failed attempts consume budget"))
            .is_err());
    }
    #[test]
    fn cached_data_cannot_bypass_freshness_check() {
        let snapshot = Snapshot {
            bars: vec![Bar::same_bar(
                Timestamp(1_000_000_000),
                1.0,
                1.0,
                1.0,
                1.0,
                1,
            )],
            fetched_at_ms: 1_000,
        };
        assert!(require_recent(&snapshot, "1m", 100_000).is_ok());
        assert!(require_recent(&snapshot, "1m", 200_000).is_err());
        assert!(require_recent(&snapshot, "1d", 5 * 86_400_000).is_err());
        assert!(require_recent(&snapshot, "1m", 0).is_err());
    }
}
