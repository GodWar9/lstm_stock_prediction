//! SignalStream trait and decoupled stream providers for backtesting.

use quant_inference::provider::PredictionProvider;
use quant_instruments::InstrumentId;
use quant_signals::{Signal, SignalCalibrator, SignalConfig};
use std::collections::VecDeque;

/// Trait defining a model-agnostic chronological stream of trading signals.
///
/// Ensures the backtester has no dependency on concrete ML models.
pub trait SignalStream: Send {
    /// Return the batch of signals available point-in-time at `as_of` timestamp.
    fn next_batch(&mut self, as_of: i64) -> Vec<Signal>;
}

/// A signal stream driven by an ONNX or ML prediction provider.
pub struct ModelSignalStream<P: PredictionProvider> {
    pub provider: P,
    pub calibrator: SignalCalibrator,
    pub instrument: InstrumentId,
    pub symbol: String,
    pub feature_history: VecDeque<Vec<f64>>,
}

impl<P: PredictionProvider> ModelSignalStream<P> {
    pub fn new(provider: P, instrument: InstrumentId, symbol: impl Into<String>) -> Self {
        Self {
            provider,
            calibrator: SignalCalibrator::new(SignalConfig::default()),
            instrument,
            symbol: symbol.into(),
            feature_history: VecDeque::new(),
        }
    }

    /// Push a new observed point-in-time feature vector.
    pub fn push_features(&mut self, features: Vec<f64>) {
        let max_lookback = self.provider.lookback();
        self.feature_history.push_back(features);
        if self.feature_history.len() > max_lookback {
            self.feature_history.pop_front();
        }
    }
}

impl<P: PredictionProvider> SignalStream for ModelSignalStream<P> {
    fn next_batch(&mut self, as_of: i64) -> Vec<Signal> {
        let lookback = self.provider.lookback();
        let num_features = self.provider.feature_schema().len();

        if self.feature_history.len() < lookback {
            return Vec::new();
        }

        // Flatten features [lookback, num_features]
        let mut flattened = Vec::with_capacity(lookback * num_features);
        for row in self.feature_history.iter().rev().take(lookback).rev() {
            flattened.extend_from_slice(row);
        }

        match self.provider.predict(&flattened, lookback, num_features) {
            Ok(pred) => {
                let sig = self
                    .calibrator
                    .calibrate(&pred, self.instrument, &self.symbol, as_of);
                vec![sig]
            }
            Err(_) => Vec::new(),
        }
    }
}

/// Manual or rule-based signal stream for baseline and model-agnostic validation.
pub struct ManualSignalStream {
    generator: Box<dyn FnMut(i64) -> Vec<Signal> + Send>,
}

impl ManualSignalStream {
    pub fn new<F>(generator: F) -> Self
    where
        F: FnMut(i64) -> Vec<Signal> + Send + 'static,
    {
        Self {
            generator: Box::new(generator),
        }
    }

    pub fn from_signals(mut signals: Vec<Signal>) -> Self {
        // Sort ascending by timestamp
        signals.sort_by_key(|s| s.as_of);
        let mut idx = 0;
        Self::new(move |as_of| {
            let mut batch = Vec::new();
            while idx < signals.len() && signals[idx].as_of <= as_of {
                batch.push(signals[idx].clone());
                idx += 1;
            }
            batch
        })
    }
}

impl SignalStream for ManualSignalStream {
    fn next_batch(&mut self, as_of: i64) -> Vec<Signal> {
        (self.generator)(as_of)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quant_signals::Direction;

    #[test]
    fn test_manual_signal_stream() {
        let sig = Signal {
            direction: Direction::Long,
            expected_return: 0.01,
            confidence: 0.8,
            horizon_bars: 1,
            instrument: InstrumentId(1),
            symbol: "AAPL".to_string(),
            model_id: "test".to_string(),
            as_of: 1000,
        };

        let mut stream = ManualSignalStream::from_signals(vec![sig.clone()]);
        assert_eq!(stream.next_batch(500).len(), 0);
        let batch = stream.next_batch(1000);
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].symbol, "AAPL");
        assert_eq!(stream.next_batch(1500).len(), 0);
    }
}
