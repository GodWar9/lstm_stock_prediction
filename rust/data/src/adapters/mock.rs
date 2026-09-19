//! In-memory and synthetic deterministic test adapters for MarketDataProvider.

use crate::provider::{DataError, MarketDataProvider};
use crate::types::{Bar, CorporateAction, Timestamp};
use chrono::{Duration, NaiveDate, TimeZone, Utc};
use quant_calendar::{TradingCalendar, UsEquityCalendar};
use std::collections::HashMap;

/// Pre-populated in-memory data provider for testing.
#[derive(Debug, Clone, Default)]
pub struct InMemoryDataProvider {
    bars: HashMap<String, Vec<Bar>>,
    actions: HashMap<String, Vec<CorporateAction>>,
}

impl InMemoryDataProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_bars(mut self, symbol: impl Into<String>, bars: Vec<Bar>) -> Self {
        self.bars.insert(symbol.into(), bars);
        self
    }

    pub fn with_actions(
        mut self,
        symbol: impl Into<String>,
        actions: Vec<CorporateAction>,
    ) -> Self {
        self.actions.insert(symbol.into(), actions);
        self
    }
}

impl MarketDataProvider for InMemoryDataProvider {
    fn fetch_ohlcv(
        &self,
        symbol: &str,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<Bar>, DataError> {
        let bars = self
            .bars
            .get(symbol)
            .ok_or_else(|| DataError::UnknownSymbol(symbol.to_string()))?;

        let start_dt = Utc.from_utc_datetime(&start.and_hms_opt(0, 0, 0).unwrap());
        let end_dt = Utc.from_utc_datetime(&end.and_hms_opt(23, 59, 59).unwrap());
        let start_ts = Timestamp::from_datetime(start_dt);
        let end_ts = Timestamp::from_datetime(end_dt);

        let filtered: Vec<Bar> = bars
            .iter()
            .filter(|b| b.timestamp >= start_ts && b.timestamp <= end_ts)
            .cloned()
            .collect();

        Ok(filtered)
    }

    fn corporate_actions(
        &self,
        symbol: &str,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<CorporateAction>, DataError> {
        let actions = self.actions.get(symbol).cloned().unwrap_or_default();
        let filtered: Vec<CorporateAction> = actions
            .into_iter()
            .filter(|a| a.effective_date >= start && a.effective_date <= end)
            .collect();
        Ok(filtered)
    }

    fn trading_calendar(&self, exchange: &str) -> Result<Box<dyn TradingCalendar>, DataError> {
        Ok(Box::new(UsEquityCalendar::new(exchange)))
    }
}

/// Deterministic synthetic data generator using simple linear congruential PRNG.
#[derive(Debug, Clone)]
pub struct SyntheticDataProvider {
    initial_price: f64,
    daily_vol: f64,
    seed: u64,
}

impl SyntheticDataProvider {
    pub fn new(initial_price: f64, daily_vol: f64, seed: u64) -> Self {
        Self {
            initial_price,
            daily_vol,
            seed,
        }
    }

    /// Pseudo-random float in [-1.0, 1.0].
    fn next_rand(&self, state: &mut u64) -> f64 {
        *state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let val = (*state >> 33) as f64 / (1u64 << 31) as f64;
        (val * 2.0) - 1.0
    }
}

impl Default for SyntheticDataProvider {
    fn default() -> Self {
        Self::new(150.0, 0.015, 42)
    }
}

impl MarketDataProvider for SyntheticDataProvider {
    fn fetch_ohlcv(
        &self,
        _symbol: &str,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<Bar>, DataError> {
        let calendar = UsEquityCalendar::default();
        let mut bars = Vec::new();
        let mut curr_date = start;
        let mut price = self.initial_price;
        let mut prng = self.seed;

        while curr_date <= end {
            if calendar.is_trading_day(curr_date) {
                if let Some(session) = calendar.session(curr_date) {
                    let r = self.next_rand(&mut prng);
                    let daily_ret = r * self.daily_vol;
                    let open = price;
                    let close = price * (1.0 + daily_ret);
                    let high = open.max(close) * (1.0 + (self.next_rand(&mut prng).abs() * 0.005));
                    let low = open.min(close) * (1.0 - (self.next_rand(&mut prng).abs() * 0.005));
                    let volume =
                        (1_000_000.0 + self.next_rand(&mut prng) * 200_000.0).max(100_000.0) as u64;

                    let ts = Timestamp::from_datetime(session.close_utc);
                    bars.push(Bar::same_bar(ts, open, high, low, close, volume));

                    price = close;
                }
            }
            curr_date += Duration::days(1);
        }

        Ok(bars)
    }

    fn corporate_actions(
        &self,
        _symbol: &str,
        _start: NaiveDate,
        _end: NaiveDate,
    ) -> Result<Vec<CorporateAction>, DataError> {
        Ok(Vec::new())
    }

    fn trading_calendar(&self, exchange: &str) -> Result<Box<dyn TradingCalendar>, DataError> {
        Ok(Box::new(UsEquityCalendar::new(exchange)))
    }
}
