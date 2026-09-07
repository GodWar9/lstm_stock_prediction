//! MACD (Moving Average Convergence Divergence) indicator.

use crate::moving_avg::Ema;
use crate::traits::Feature;
use crate::window::BarWindow;

/// MACD Line: fast EMA minus slow EMA.
#[derive(Debug)]
pub struct MacdLine {
    fast_period: usize,
    slow_period: usize,
}

impl MacdLine {
    pub fn new(fast_period: usize, slow_period: usize) -> Self {
        assert!(
            fast_period < slow_period,
            "fast_period must be < slow_period"
        );
        Self {
            fast_period,
            slow_period,
        }
    }

    pub fn standard() -> Self {
        Self::new(12, 26)
    }
}

impl Feature for MacdLine {
    fn name(&self) -> &str {
        "macd_line"
    }

    fn version(&self) -> u32 {
        1
    }

    fn lookback(&self) -> usize {
        self.slow_period
    }

    fn compute(&self, window: &BarWindow) -> Option<f64> {
        let closes = window.closes();
        if closes.len() < self.slow_period {
            return None;
        }

        let fast_ema = Ema::ema_over(&closes, self.fast_period)?;
        let slow_ema = Ema::ema_over(&closes, self.slow_period)?;
        Some(fast_ema - slow_ema)
    }
}

/// Full MACD output containing line, signal, and histogram.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MacdOutput {
    pub macd: f64,
    pub signal: f64,
    pub histogram: f64,
}

/// Full MACD Calculator computing MACD, Signal, and Histogram.
#[derive(Debug)]
pub struct Macd {
    pub fast_period: usize,
    pub slow_period: usize,
    pub signal_period: usize,
}

impl Macd {
    pub fn new(fast: usize, slow: usize, signal: usize) -> Self {
        assert!(fast < slow, "fast period must be < slow period");
        assert!(signal > 0, "signal period must be > 0");
        Self {
            fast_period: fast,
            slow_period: slow,
            signal_period: signal,
        }
    }

    pub fn standard() -> Self {
        Self::new(12, 26, 9)
    }

    pub fn compute_full(&self, window: &BarWindow) -> Option<MacdOutput> {
        let closes = window.closes();
        let total_lookback = self.slow_period + self.signal_period;
        if closes.len() < total_lookback {
            return None;
        }

        // Calculate series of MACD line values over the needed window
        let mut macd_series = Vec::new();
        for end_idx in self.slow_period..=closes.len() {
            let slice = &closes[..end_idx];
            let fast = Ema::ema_over(slice, self.fast_period)?;
            let slow = Ema::ema_over(slice, self.slow_period)?;
            macd_series.push(fast - slow);
        }

        let current_macd = *macd_series.last()?;
        let signal = Ema::ema_over(&macd_series, self.signal_period)?;
        let histogram = current_macd - signal;

        Some(MacdOutput {
            macd: current_macd,
            signal,
            histogram,
        })
    }
}

impl Feature for Macd {
    fn name(&self) -> &str {
        "macd"
    }

    fn version(&self) -> u32 {
        1
    }

    fn lookback(&self) -> usize {
        self.slow_period + self.signal_period
    }

    /// Computes and returns the MACD histogram by default.
    fn compute(&self, window: &BarWindow) -> Option<f64> {
        self.compute_full(window).map(|out| out.histogram)
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
    fn test_macd_line_computation() {
        let closes: Vec<f64> = (1..=30).map(|x| x as f64).collect();
        let w = make_window(&closes);
        let macd_line = MacdLine::new(5, 10);
        let val = macd_line.compute(&w).unwrap();
        // Uptrending prices means fast EMA > slow EMA, so MACD line > 0
        assert!(val > 0.0);
    }

    #[test]
    fn test_macd_full_histogram() {
        let closes: Vec<f64> = (1..=40).map(|x| x as f64).collect();
        let w = make_window(&closes);
        let macd = Macd::new(5, 10, 5);
        let output = macd.compute_full(&w).unwrap();
        assert_eq!(output.histogram, output.macd - output.signal);
    }
}
