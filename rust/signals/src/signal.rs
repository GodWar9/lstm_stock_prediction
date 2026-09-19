//! Core Signal structures and prediction-to-signal calibration.

use quant_inference::provider::Prediction;
use quant_instruments::InstrumentId;
use serde::{Deserialize, Serialize};

/// Trading direction recommended by a signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    Long,
    Short,
    Flat,
}

impl Direction {
    /// Return the numeric direction multiplier: +1.0 for Long, -1.0 for Short, 0.0 for Flat.
    pub fn signum(&self) -> f64 {
        match self {
            Self::Long => 1.0,
            Self::Short => -1.0,
            Self::Flat => 0.0,
        }
    }

    pub fn is_active(&self) -> bool {
        !matches!(self, Self::Flat)
    }
}

/// Rich trading signal emitted by quantitative models and strategies.
///
/// Designed to satisfy traceability, multi-horizon execution, and derivatives compatibility.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Signal {
    /// Recommended position direction (Long, Short, Flat).
    pub direction: Direction,
    /// Model predicted expected forward return (e.g. +0.012 = +1.2%).
    pub expected_return: f64,
    /// Calibrated model confidence in [0.0, 1.0].
    pub confidence: f64,
    /// Prediction observation horizon in bars (e.g., 1 bar forward).
    pub horizon_bars: usize,
    /// Target instrument identifier.
    pub instrument: InstrumentId,
    /// Instrument ticker symbol.
    pub symbol: String,
    /// Model identifier for provenance auditability.
    pub model_id: String,
    /// Point-in-time timestamp (unix nanoseconds or milliseconds UTC).
    pub as_of: i64,
}

impl Signal {
    /// Create a new flat/neutral signal.
    pub fn flat(
        instrument: InstrumentId,
        symbol: impl Into<String>,
        model_id: impl Into<String>,
        as_of: i64,
    ) -> Self {
        Self {
            direction: Direction::Flat,
            expected_return: 0.0,
            confidence: 0.0,
            horizon_bars: 1,
            instrument,
            symbol: symbol.into(),
            model_id: model_id.into(),
            as_of,
        }
    }
}

/// Configuration parameters for calibrating predictions into signals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalConfig {
    /// Minimum expected return required to trigger a directional position (deadband threshold).
    pub return_threshold: f64,
    /// Default forecast horizon in bars.
    pub default_horizon_bars: usize,
    /// Scaling parameter for tanh-based confidence calibration when confidence is not provided by the model.
    pub confidence_scale: f64,
}

impl Default for SignalConfig {
    fn default() -> Self {
        Self {
            return_threshold: 0.0002, // 2 bps minimum expectation
            default_horizon_bars: 1,
            confidence_scale: 0.01, // 1% expected return reaches ~76% confidence
        }
    }
}

/// Converts raw model predictions into calibrated actionable trading signals.
#[derive(Debug, Clone)]
pub struct SignalCalibrator {
    config: SignalConfig,
}

impl SignalCalibrator {
    pub fn new(config: SignalConfig) -> Self {
        Self { config }
    }

    /// Calibrate a raw model `Prediction` into a structured `Signal`.
    pub fn calibrate(
        &self,
        pred: &Prediction,
        instrument: InstrumentId,
        symbol: impl Into<String>,
        as_of: i64,
    ) -> Signal {
        let expected_return = pred.value;

        // Determine direction based on return threshold deadband
        let direction = if expected_return > self.config.return_threshold {
            Direction::Long
        } else if expected_return < -self.config.return_threshold {
            Direction::Short
        } else {
            Direction::Flat
        };

        // Calibrate confidence: use model-supplied confidence if present, or compute from return magnitude
        let confidence = match pred.confidence {
            Some(c) => c.clamp(0.0, 1.0),
            None => {
                if direction == Direction::Flat {
                    0.0
                } else {
                    let normalized =
                        (expected_return.abs() / self.config.confidence_scale.max(1e-6)).tanh();
                    normalized.clamp(0.0, 1.0)
                }
            }
        };

        Signal {
            direction,
            expected_return,
            confidence,
            horizon_bars: self.config.default_horizon_bars,
            instrument,
            symbol: symbol.into(),
            model_id: pred.model_id.clone(),
            as_of,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direction_signum() {
        assert_eq!(Direction::Long.signum(), 1.0);
        assert_eq!(Direction::Short.signum(), -1.0);
        assert_eq!(Direction::Flat.signum(), 0.0);
    }

    #[test]
    fn test_signal_calibration_long() {
        let calibrator = SignalCalibrator::new(SignalConfig::default());
        let pred = Prediction {
            value: 0.015,
            confidence: None,
            model_id: "lstm_v1".to_string(),
            as_of: None,
        };

        let sig = calibrator.calibrate(&pred, InstrumentId(1), "AAPL", 1700000000);
        assert_eq!(sig.direction, Direction::Long);
        assert!(sig.confidence > 0.5);
        assert_eq!(sig.symbol, "AAPL");
        assert_eq!(sig.model_id, "lstm_v1");
    }

    #[test]
    fn test_signal_calibration_flat_deadband() {
        let calibrator = SignalCalibrator::new(SignalConfig {
            return_threshold: 0.001,
            ..Default::default()
        });
        let pred = Prediction {
            value: 0.0005, // Below threshold
            confidence: None,
            model_id: "lstm_v1".to_string(),
            as_of: None,
        };

        let sig = calibrator.calibrate(&pred, InstrumentId(1), "MSFT", 1700000000);
        assert_eq!(sig.direction, Direction::Flat);
        assert_eq!(sig.confidence, 0.0);
    }

    #[test]
    fn test_signal_calibration_short() {
        let calibrator = SignalCalibrator::new(SignalConfig::default());
        let pred = Prediction {
            value: -0.02,
            confidence: Some(0.85),
            model_id: "lstm_v1".to_string(),
            as_of: None,
        };

        let sig = calibrator.calibrate(&pred, InstrumentId(2), "NVDA", 1700000000);
        assert_eq!(sig.direction, Direction::Short);
        assert_eq!(sig.confidence, 0.85);
    }
}
