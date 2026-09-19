//! Relative Strength Index (RSI) using Wilder's smoothing.

use crate::traits::Feature;
use crate::window::BarWindow;

/// Relative Strength Index indicator.
#[derive(Debug)]
pub struct Rsi {
    period: usize,
}

impl Rsi {
    pub fn new(period: usize) -> Self {
        assert!(period > 0, "RSI period must be > 0");
        Self { period }
    }

    /// Calculate RSI from a slice of close prices.
    pub fn compute_from_closes(closes: &[f64], period: usize) -> Option<f64> {
        if closes.len() <= period {
            return None;
        }

        let mut changes = Vec::with_capacity(closes.len() - 1);
        for i in 1..closes.len() {
            changes.push(closes[i] - closes[i - 1]);
        }

        if changes.len() < period {
            return None;
        }

        // Initial averages (SMA of first `period` gains/losses)
        let mut avg_gain = 0.0;
        let mut avg_loss = 0.0;
        for &diff in &changes[..period] {
            if diff > 0.0 {
                avg_gain += diff;
            } else {
                avg_loss += -diff;
            }
        }
        avg_gain /= period as f64;
        avg_loss /= period as f64;

        // Wilder's smoothing for the remainder
        let period_f = period as f64;
        for &diff in &changes[period..] {
            let gain = if diff > 0.0 { diff } else { 0.0 };
            let loss = if diff < 0.0 { -diff } else { 0.0 };

            avg_gain = (avg_gain * (period_f - 1.0) + gain) / period_f;
            avg_loss = (avg_loss * (period_f - 1.0) + loss) / period_f;
        }

        if avg_loss == 0.0 {
            if avg_gain == 0.0 {
                Some(50.0) // No movement
            } else {
                Some(100.0) // Pure gains
            }
        } else {
            let rs = avg_gain / avg_loss;
            Some(100.0 - (100.0 / (1.0 + rs)))
        }
    }
}

impl Feature for Rsi {
    fn name(&self) -> &str {
        "rsi"
    }

    fn version(&self) -> u32 {
        1
    }

    fn lookback(&self) -> usize {
        self.period + 1 // Needs at least period + 1 prices for differences
    }

    fn compute(&self, window: &BarWindow) -> Option<f64> {
        let closes = window.closes();
        Self::compute_from_closes(&closes, self.period)
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
    fn test_rsi_all_gains() {
        let closes = vec![10.0, 11.0, 12.0, 13.0, 14.0, 15.0];
        let rsi = Rsi::new(5);
        let w = make_window(&closes);
        let val = rsi.compute(&w).unwrap();
        assert!((val - 100.0).abs() < 1e-6);
    }

    #[test]
    fn test_rsi_flat() {
        let closes = vec![10.0, 10.0, 10.0, 10.0, 10.0, 10.0];
        let rsi = Rsi::new(5);
        let w = make_window(&closes);
        let val = rsi.compute(&w).unwrap();
        assert!((val - 50.0).abs() < 1e-6);
    }

    #[test]
    fn test_rsi_insufficient_lookback() {
        let closes = vec![10.0, 11.0, 12.0];
        let rsi = Rsi::new(5);
        let w = make_window(&closes);
        assert!(rsi.compute(&w).is_none());
    }

    #[test]
    fn test_rsi_oscillation_range() {
        let closes = vec![
            44.34, 44.09, 44.15, 43.61, 44.33, 44.83, 45.10, 45.42, 45.84, 46.08, 45.89, 46.03,
            45.61, 46.28, 46.28, 46.00,
        ];
        let rsi = Rsi::new(14);
        let w = make_window(&closes);
        let val = rsi.compute(&w).unwrap();
        assert!(val > 0.0 && val < 100.0);
    }
}
