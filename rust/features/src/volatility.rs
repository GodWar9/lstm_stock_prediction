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
        let w_len = window.len();
        if w_len < self.lookback() {
            return None;
        }

        let start_idx = w_len - (self.period + 1);
        let mut prev_close = window.get(start_idx)?.close;
        if prev_close <= 0.0 {
            return None;
        }

        let mut mean = 0.0;
        let mut m2 = 0.0;

        for step in 1..=self.period {
            let curr_close = window.get(start_idx + step)?.close;
            if curr_close <= 0.0 {
                return None;
            }
            let r = (curr_close / prev_close).ln();
            prev_close = curr_close;

            let delta = r - mean;
            mean += delta / (step as f64);
            let delta2 = r - mean;
            m2 += delta * delta2;
        }

        let variance = m2 / (self.period as f64 - 1.0);
        Some(variance.sqrt() * self.annualization_factor)
    }
}

/// Parkinson High-Low Volatility Estimator.
/// Up to 5x more statistically efficient than close-to-close volatility by capturing intraday price dispersion.
#[derive(Debug)]
pub struct ParkinsonVolatility {
    period: usize,
    annualization_factor: f64,
}

impl ParkinsonVolatility {
    pub fn new(period: usize, annualization_factor: f64) -> Self {
        assert!(period > 0, "period must be > 0");
        Self {
            period,
            annualization_factor,
        }
    }

    pub fn daily(period: usize) -> Self {
        Self::new(period, 252.0f64.sqrt())
    }
}

impl Feature for ParkinsonVolatility {
    fn name(&self) -> &str {
        "parkinson_volatility"
    }

    fn version(&self) -> u32 {
        1
    }

    fn lookback(&self) -> usize {
        self.period
    }

    fn compute(&self, window: &BarWindow) -> Option<f64> {
        let w_len = window.len();
        if w_len < self.period {
            return None;
        }

        let start_idx = w_len - self.period;
        let mut sum_hl_sq = 0.0;

        for i in 0..self.period {
            let bar = window.get(start_idx + i)?;
            if bar.low <= 0.0 || bar.high <= 0.0 || bar.high < bar.low {
                return None;
            }
            let hl_ratio = (bar.high / bar.low).ln();
            sum_hl_sq += hl_ratio * hl_ratio;
        }

        let factor = 1.0 / (4.0 * 2.0_f64.ln());
        let variance = (factor * sum_hl_sq) / self.period as f64;
        Some(variance.sqrt() * self.annualization_factor)
    }
}

/// Garman-Klass Open-High-Low-Close Volatility Estimator.
/// Up to 8x more statistically efficient than close-to-close volatility by incorporating opening gaps and intraday ranges.
#[derive(Debug)]
pub struct GarmanKlassVolatility {
    period: usize,
    annualization_factor: f64,
}

impl GarmanKlassVolatility {
    pub fn new(period: usize, annualization_factor: f64) -> Self {
        assert!(period > 0, "period must be > 0");
        Self {
            period,
            annualization_factor,
        }
    }

    pub fn daily(period: usize) -> Self {
        Self::new(period, 252.0f64.sqrt())
    }
}

impl Feature for GarmanKlassVolatility {
    fn name(&self) -> &str {
        "garman_klass_volatility"
    }

    fn version(&self) -> u32 {
        1
    }

    fn lookback(&self) -> usize {
        self.period
    }

    fn compute(&self, window: &BarWindow) -> Option<f64> {
        let w_len = window.len();
        if w_len < self.period {
            return None;
        }

        let start_idx = w_len - self.period;
        let mut sum_term = 0.0;
        let c2 = 2.0_f64.ln() * 2.0 - 1.0;

        for i in 0..self.period {
            let bar = window.get(start_idx + i)?;
            if bar.low <= 0.0 || bar.high <= 0.0 || bar.open <= 0.0 || bar.close <= 0.0 {
                return None;
            }
            let hl_ratio = (bar.high / bar.low).ln();
            let co_ratio = (bar.close / bar.open).ln();
            sum_term += 0.5 * hl_ratio * hl_ratio - c2 * co_ratio * co_ratio;
        }

        let variance = (sum_term / self.period as f64).max(0.0);
        Some(variance.sqrt() * self.annualization_factor)
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

    #[test]
    fn test_parkinson_and_garman_klass_volatility() {
        let mut w = BarWindow::new(5);
        for i in 0..5 {
            let ts = Timestamp(i as i64 * 86_400_000_000_000);
            w.push(Bar::new(
                ts,
                ts,
                100.0 + i as f64,
                105.0 + i as f64,
                98.0 + i as f64,
                102.0 + i as f64,
                1000,
                false,
            ));
        }

        let pv = ParkinsonVolatility::daily(5);
        let p_val = pv.compute(&w);
        assert!(p_val.is_some());
        assert!(p_val.unwrap() > 0.0);

        let gk = GarmanKlassVolatility::daily(5);
        let gk_val = gk.compute(&w);
        assert!(gk_val.is_some());
        assert!(gk_val.unwrap() > 0.0);
    }
}
