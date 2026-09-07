//! Feature trait and identifier types.

use std::time::Duration;

/// Unique versioned identifier for a feature.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FeatureId {
    pub name: String,
    pub version: u32,
}

impl FeatureId {
    pub fn new(name: impl Into<String>, version: u32) -> Self {
        Self {
            name: name.into(),
            version,
        }
    }
}

impl std::fmt::Display for FeatureId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@v{}", self.name, self.version)
    }
}

/// Core trait representing a single feature computation.
///
/// Every indicator (RSI, ATR, MACD, etc.) implements this trait.
/// `version()` must be bumped whenever `compute()` logic changes, so the
/// `FeatureStore` never silently overwrites historical cached values.
pub trait Feature: Send + Sync + std::fmt::Debug {
    /// Human-readable feature name.
    fn name(&self) -> &str;

    /// Monotonically increasing version — bump on any formula change.
    fn version(&self) -> u32;

    /// Minimum number of bars required before this feature can produce a valid output.
    fn lookback(&self) -> usize;

    /// Computes the feature value from the current bar window.
    /// Returns `None` if insufficient data (warmup period).
    fn compute(&self, window: &crate::window::BarWindow) -> Option<f64>;

    /// How much later this feature's value becomes available relative to the bar's close.
    /// Zero for same-bar features (the common case).
    fn availability_offset(&self) -> Duration {
        Duration::from_secs(0)
    }

    /// Composite identifier combining name and version.
    fn id(&self) -> FeatureId {
        FeatureId::new(self.name(), self.version())
    }
}
