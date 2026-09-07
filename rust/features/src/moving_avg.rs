//! Moving average indicators: SMA and EMA.

use crate::traits::Feature;
use crate::window::BarWindow;

/// Simple Moving Average.
#[derive(Debug)]
pub struct Sma {
    period: usize,
}

impl Sma {
    pub fn new(period: usize) -> Self {
        assert!(period > 0, "SMA period must be > 0");
        Self { period }
    }
}

impl Feature for Sma {
    fn name(&self) -> &str {
        "sma"
    }

    fn version(&self) -> u32 {
        1
    }

    fn lookback(&self) -> usize {
        self.period
    }

    fn compute(&self, window: &BarWindow) -> Option<f64> {
        if window.len() < self.period {
            return None;
        }
        let n = window.len();
        let sum: f64 = (n - self.period..n)
            .filter_map(|i| window.get(i).map(|b| b.close))
            .sum();
        Some(sum / self.period as f64)
    }
}

/// Exponential Moving Average.
///
/// Uses the standard multiplier: `2 / (period + 1)`.
/// Seeded from the first SMA once enough bars are accumulated.
#[derive(Debug)]
pub struct Ema {
    period: usize,
}

impl Ema {
    pub fn new(period: usize) -> Self {
        assert!(period > 0, "EMA period must be > 0");
        Self { period }
    }

    /// Compute EMA over an arbitrary slice of f64 values.
    /// Public so other indicators (MACD, Bollinger) can reuse it.
    pub fn ema_over(values: &[f64], period: usize) -> Option<f64> {
        if values.len() < period {
            return None;
        }
        let multiplier = 2.0 / (period as f64 + 1.0);
        // Seed with SMA of the first `period` values.
        let seed: f64 = values[..period].iter().sum::<f64>() / period as f64;
        let result = values[period..].iter().fold(seed, |prev_ema, &val| {
            (val - prev_ema) * multiplier + prev_ema
        });
        Some(result)
    }
}

impl Feature for Ema {
    fn name(&self) -> &str {
        "ema"
    }

    fn version(&self) -> u32 {
        1
    }

    fn lookback(&self) -> usize {
        self.period
    }

    fn compute(&self, window: &BarWindow) -> Option<f64> {
        let closes = window.closes();
        if closes.len() < self.period {
            return None;
        }
        Self::ema_over(&closes, self.period)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quant_data::{Bar, Timestamp};

    fn bar(close: f64) -> Bar {
        Bar::same_bar(Timestamp(0), close - 1.0, close + 1.0, close - 2.0, close, 1000)
    }

    fn make_window(closes: &[f64]) -> BarWindow {
        let mut w = BarWindow::new(closes.len());
        for &c in closes {
            w.push(bar(c));
        }
        w
    }

    #[test]
    fn test_sma_basic() {
        let w = make_window(&[10.0, 20.0, 30.0, 40.0, 50.0]);
        let sma = Sma::new(3);
        // SMA(3) over last 3 bars: (30 + 40 + 50) / 3 = 40
        let val = sma.compute(&w).unwrap();
        assert!((val - 40.0).abs() < 1e-10);
    }

    #[test]
    fn test_sma_insufficient_data() {
        let w = make_window(&[10.0, 20.0]);
        let sma = Sma::new(5);
        assert!(sma.compute(&w).is_none());
    }

    #[test]
    fn test_ema_basic() {
        // Manually computed EMA(3) over [2, 4, 6, 8, 10]:
        // Seed SMA = (2+4+6)/3 = 4.0
        // mult = 2/(3+1) = 0.5
        // EMA after 8: (8-4)*0.5 + 4 = 6.0
        // EMA after 10: (10-6)*0.5 + 6 = 8.0
        let w = make_window(&[2.0, 4.0, 6.0, 8.0, 10.0]);
        let ema = Ema::new(3);
        let val = ema.compute(&w).unwrap();
        assert!((val - 8.0).abs() < 1e-10);
    }

    #[test]
    fn test_ema_over_static() {
        let vals = vec![2.0, 4.0, 6.0, 8.0, 10.0];
        let val = Ema::ema_over(&vals, 3).unwrap();
        assert!((val - 8.0).abs() < 1e-10);
    }
}
