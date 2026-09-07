//! Average True Range (ATR) indicator.

use crate::traits::Feature;
use crate::window::BarWindow;

/// Average True Range indicator using Wilder's smoothing.
#[derive(Debug)]
pub struct Atr {
    period: usize,
}

impl Atr {
    pub fn new(period: usize) -> Self {
        assert!(period > 0, "ATR period must be > 0");
        Self { period }
    }

    /// Calculate True Range between current bar and previous bar's close.
    pub fn true_range(current_high: f64, current_low: f64, prev_close: f64) -> f64 {
        let hl = current_high - current_low;
        let hc = (current_high - prev_close).abs();
        let lc = (current_low - prev_close).abs();
        hl.max(hc).max(lc)
    }
}

impl Feature for Atr {
    fn name(&self) -> &str {
        "atr"
    }

    fn version(&self) -> u32 {
        1
    }

    fn lookback(&self) -> usize {
        self.period + 1 // Needs prior close for initial TR
    }

    fn compute(&self, window: &BarWindow) -> Option<f64> {
        if window.len() < self.lookback() {
            return None;
        }

        let mut trs = Vec::with_capacity(window.len() - 1);
        for i in 1..window.len() {
            let curr = window.get(i)?;
            let prev = window.get(i - 1)?;
            trs.push(Self::true_range(curr.high, curr.low, prev.close));
        }

        if trs.len() < self.period {
            return None;
        }

        // Wilder's smoothing
        let period_f = self.period as f64;
        let mut atr: f64 = trs[..self.period].iter().sum::<f64>() / period_f;

        for &tr in &trs[self.period..] {
            atr = (atr * (period_f - 1.0) + tr) / period_f;
        }

        Some(atr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quant_data::{Bar, Timestamp};

    #[test]
    fn test_true_range_formula() {
        // High 12, Low 8, Prev close 10 => max(4, 2, 2) = 4
        assert_eq!(Atr::true_range(12.0, 8.0, 10.0), 4.0);
        // Gap up: High 20, Low 15, Prev close 10 => max(5, 10, 5) = 10
        assert_eq!(Atr::true_range(20.0, 15.0, 10.0), 10.0);
    }

    #[test]
    fn test_atr_window_computation() {
        let mut w = BarWindow::new(5);
        w.push(Bar::same_bar(Timestamp(1), 10.0, 12.0, 9.0, 11.0, 100));
        w.push(Bar::same_bar(Timestamp(2), 11.0, 13.0, 10.0, 12.0, 100));
        w.push(Bar::same_bar(Timestamp(3), 12.0, 14.0, 11.0, 13.0, 100));
        w.push(Bar::same_bar(Timestamp(4), 13.0, 15.0, 12.0, 14.0, 100));

        let atr = Atr::new(3);
        let val = atr.compute(&w);
        assert!(val.is_some());
        assert!(val.unwrap() > 0.0);
    }
}
