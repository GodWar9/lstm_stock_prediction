//! Bollinger Bands indicator.

use crate::traits::Feature;
use crate::window::BarWindow;

/// Output of Bollinger Bands calculation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BollingerOutput {
    pub middle: f64,
    pub upper: f64,
    pub lower: f64,
    pub bandwidth: f64,
    pub percent_b: f64,
}

/// Bollinger Bands indicator.
#[derive(Debug)]
pub struct BollingerBands {
    pub period: usize,
    pub num_std: f64,
}

impl BollingerBands {
    pub fn new(period: usize, num_std: f64) -> Self {
        assert!(period > 1, "period must be > 1");
        assert!(num_std > 0.0, "num_std must be > 0.0");
        Self { period, num_std }
    }

    pub fn standard() -> Self {
        Self::new(20, 2.0)
    }

    pub fn compute_bands(&self, window: &BarWindow) -> Option<BollingerOutput> {
        let closes = window.closes();
        if closes.len() < self.period {
            return None;
        }

        let slice = &closes[closes.len() - self.period..];
        let n = self.period as f64;
        let mean = slice.iter().sum::<f64>() / n;
        let variance = slice.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / n;
        let std_dev = variance.sqrt();

        let upper = mean + self.num_std * std_dev;
        let lower = mean - self.num_std * std_dev;
        let bandwidth = if mean > 0.0 { (upper - lower) / mean } else { 0.0 };

        let latest = *slice.last()?;
        let percent_b = if (upper - lower).abs() > 1e-10 {
            (latest - lower) / (upper - lower)
        } else {
            0.5
        };

        Some(BollingerOutput {
            middle: mean,
            upper,
            lower,
            bandwidth,
            percent_b,
        })
    }
}

impl Feature for BollingerBands {
    fn name(&self) -> &str {
        "bollinger_bandwidth"
    }

    fn version(&self) -> u32 {
        1
    }

    fn lookback(&self) -> usize {
        self.period
    }

    fn compute(&self, window: &BarWindow) -> Option<f64> {
        self.compute_bands(window).map(|out| out.bandwidth)
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
    fn test_bollinger_bands_calculation() {
        let closes = vec![10.0, 10.0, 10.0, 10.0, 10.0];
        let bb = BollingerBands::new(5, 2.0);
        let w = make_window(&closes);
        let out = bb.compute_bands(&w).unwrap();
        assert_eq!(out.middle, 10.0);
        assert_eq!(out.upper, 10.0);
        assert_eq!(out.lower, 10.0);
        assert_eq!(out.bandwidth, 0.0);
        assert_eq!(out.percent_b, 0.5);
    }

    #[test]
    fn test_bollinger_bands_variance() {
        let closes = vec![10.0, 20.0, 10.0, 20.0, 10.0];
        let bb = BollingerBands::new(5, 2.0);
        let w = make_window(&closes);
        let out = bb.compute_bands(&w).unwrap();
        assert!(out.upper > out.middle);
        assert!(out.lower < out.middle);
        assert!(out.bandwidth > 0.0);
    }
}
