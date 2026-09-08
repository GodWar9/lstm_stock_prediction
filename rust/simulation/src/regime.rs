//! Market regime categorization and detection.

use serde::{Deserialize, Serialize};

/// Market regime classification labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Regime {
    Bull,
    Bear,
    HighVol,
    LowVol,
    Trending,
    MeanReverting,
    Crisis,
}

impl std::fmt::Display for Regime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bull => write!(f, "Bull"),
            Self::Bear => write!(f, "Bear"),
            Self::HighVol => write!(f, "HighVol"),
            Self::LowVol => write!(f, "LowVol"),
            Self::Trending => write!(f, "Trending"),
            Self::MeanReverting => write!(f, "MeanReverting"),
            Self::Crisis => write!(f, "Crisis"),
        }
    }
}

/// Trait for detecting the active market regime from recent market states.
pub trait RegimeDetector: Send + Sync {
    /// Label the active market regime given a recent series of returns.
    fn label(&self, returns: &[f64]) -> Regime;
}

/// Rule-based market regime detector using rolling return and volatility heuristics.
#[derive(Debug, Clone, Default)]
pub struct RuleBasedRegimeDetector {
    pub high_vol_threshold: f64,
    pub crisis_drawdown_threshold: f64,
}

impl RuleBasedRegimeDetector {
    pub fn new(high_vol_threshold: f64, crisis_drawdown_threshold: f64) -> Self {
        Self {
            high_vol_threshold,
            crisis_drawdown_threshold,
        }
    }
}

impl RegimeDetector for RuleBasedRegimeDetector {
    fn label(&self, returns: &[f64]) -> Regime {
        if returns.len() < 5 {
            return Regime::MeanReverting;
        }

        let n = returns.len() as f64;
        let mean = returns.iter().sum::<f64>() / n;
        let variance = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (n - 1.0);
        let annualized_vol = variance.sqrt() * (252.0_f64).sqrt();

        // Check for acute crisis (large single-day crash)
        let min_return = returns.iter().copied().fold(0.0_f64, f64::min);
        if min_return < -0.06 || annualized_vol > 0.45 {
            return Regime::Crisis;
        }

        if annualized_vol > self.high_vol_threshold.max(0.25) {
            return Regime::HighVol;
        }

        if mean > 0.0008 {
            Regime::Bull
        } else if mean < -0.0008 {
            Regime::Bear
        } else if annualized_vol < 0.10 {
            Regime::LowVol
        } else {
            Regime::MeanReverting
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regime_bull_detection() {
        let detector = RuleBasedRegimeDetector::default();
        let returns = vec![0.005, 0.004, 0.006, 0.003, 0.005];
        assert_eq!(detector.label(&returns), Regime::Bull);
    }

    #[test]
    fn test_regime_crisis_detection() {
        let detector = RuleBasedRegimeDetector::default();
        let returns = vec![0.001, -0.07, 0.002, -0.01, 0.001];
        assert_eq!(detector.label(&returns), Regime::Crisis);
    }
}
