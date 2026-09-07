//! Rolling volatility and log returns features.

use crate::traits::Feature;
use crate::window::BarWindow;

/// Rolling Log Returns: ln(Close_t / Close_{t-lookback}).
#[derive(Debug)]
pub struct LogReturn {
    lag: usize,
}

impl LogReturn {
    pub fn new(lag: usize) -> Self {
        assert!(lag > 0, "lag must be > 0");
        Self { lag }
    }
}

impl Feature for LogReturn {
    fn name(&self) -> &str {
        "log_return"
    }

    fn version(&self) -> u32 {
        1
    }

    fn lookback(&self) -> usize {
        self.lag + 1
    }

    fn compute(&self, window: &BarWindow) -> Option<f64> {
        if window.len() < self.lookback() {
            return None;
        }
        let curr_close = window.latest()?.close;
        let prev_close = window.get(window.len() - 1 - self.lag)?.close;
        if prev_close <= 0.0 || curr_close <= 0.0 {
            return None;
        }
        Some((curr_close / prev_close).ln())
    }
}

/// Annualized Rolling Volatility of 1-period log returns.
#[derive(Debug)]
pub struct RollingVolatility {
    period: usize,
    annualization_factor: f64,
}

impl RollingVolatility {
    pub fn new(period: usize, annualization_factor: f64) -> Self {
        assert!(period > 1, "period must be > 1");
        Self {
            period,
            annualization_factor,
        }
    }

    pub fn daily(period: usize) -> Self {
        // 252 trading days per year
        Self::new(period, 252.0f64.sqrt())
    }
}

impl Feature for RollingVolatility {
    fn name(&self) -> &str {
        "rolling_volatility"
    }

    fn version(&self) -> u32 {
        1
    }

    fn lookback(&self) -> usize {
        self.period + 1
    }

    fn compute(&self, window: &BarWindow) -> Option<f64> {
        let closes = window.closes();
        if closes.len() < self.lookback() {
            return None;
        }

        let slice = &closes[closes.len() - (self.period + 1)..];
        let mut returns = Vec::with_capacity(self.period);
        for i in 1..slice.len() {
            if slice[i - 1] <= 0.0 || slice[i] <= 0.0 {
                return None;
            }
            returns.push((slice[i] / slice[i - 1]).ln());
        }

        let n = returns.len() as f64;
        let mean = returns.iter().sum::<f64>() / n;
        let variance = returns.iter().map(|&r| (r - mean).powi(2)).sum::<f64>() / (n - 1.0);
        let std_dev = variance.sqrt();

        Some(std_dev * self.annualization_factor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quant_data::{Bar, Timestamp};

    fn make_window(closes: &[f64]) -> BarWindow {
        let mut w = BarWindow::new(closes.len());
        for (i, &c) in closes.iter().enumerate() {
            w.push(Bar::same_bar(Timestamp(i as i64), c, c, c, c, 100));
        }
        w
    }

    #[test]
    fn test_log_return() {
        let closes = vec![100.0, 105.0];
        let w = make_window(&closes);
        let lr = LogReturn::new(1);
        let val = lr.compute(&w).unwrap();
        let expected = (105.0f64 / 100.0).ln();
        assert!((val - expected).abs() < 1e-8);
    }

    #[test]
    fn test_rolling_volatility() {
        let closes = vec![100.0, 102.0, 101.0, 103.0, 102.0, 104.0];
        let w = make_window(&closes);
        let vol = RollingVolatility::daily(5);
        let val = vol.compute(&w);
        assert!(val.is_some());
        assert!(val.unwrap() > 0.0);
    }
}
