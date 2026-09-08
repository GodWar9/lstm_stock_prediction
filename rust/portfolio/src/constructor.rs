//! Portfolio construction: Volatility-targeted position sizing and constraint satisfaction.

use crate::constraints::{LongShortMode, PortfolioConstraints};
use crate::portfolio::Portfolio;
use crate::position::{TargetPosition, TargetPositions};
use quant_instruments::InstrumentId;
use quant_signals::{Direction, Signal};
use std::collections::HashMap;

/// Trait for turning signals into executable target portfolio allocations.
pub trait PortfolioConstructor: Send + Sync {
    /// Compute target allocations given active signals, current portfolio, market prices, and constraints.
    fn target_positions(
        &self,
        signals: &[Signal],
        current: &Portfolio,
        prices: &HashMap<InstrumentId, f64>,
        constraints: &PortfolioConstraints,
    ) -> TargetPositions;
}

/// Volatility-targeted portfolio constructor with constraint clipping and drawdown de-risking.
#[derive(Debug, Clone, Default)]
pub struct VolatilityTargetedConstructor {
    /// Assumed asset annualized volatility fallback when dynamic vol is unavailable.
    pub default_asset_vol: f64,
}

impl VolatilityTargetedConstructor {
    pub fn new(default_asset_vol: f64) -> Self {
        Self { default_asset_vol }
    }
}

impl PortfolioConstructor for VolatilityTargetedConstructor {
    fn target_positions(
        &self,
        signals: &[Signal],
        current: &Portfolio,
        prices: &HashMap<InstrumentId, f64>,
        constraints: &PortfolioConstraints,
    ) -> TargetPositions {
        let nav = current.nav();
        let as_of = signals.first().map(|s| s.as_of).unwrap_or(0);
        let mut target_positions = TargetPositions::new(as_of);

        if signals.is_empty() || nav <= 1e-8 {
            return target_positions;
        }

        let vol_scalar = match constraints.volatility_target {
            Some(target_vol) => {
                let asset_vol = self.default_asset_vol.max(0.01);
                (target_vol / asset_vol).clamp(0.2, 2.5)
            }
            None => 1.0,
        };

        // Drawdown de-risking check
        let is_derisking = current.current_drawdown() > constraints.drawdown_derisk_threshold;
        let derisk_scalar = if is_derisking {
            constraints.drawdown_derisk_multiplier.clamp(0.1, 1.0)
        } else {
            1.0
        };

        // 1. Calculate raw target weight per signal
        let mut raw_weights: Vec<(InstrumentId, String, f64)> = Vec::with_capacity(signals.len());
        for sig in signals {
            let dir_mult = match sig.direction {
                Direction::Long => 1.0,
                Direction::Short => match constraints.long_short_mode {
                    LongShortMode::LongOnly => 0.0,
                    _ => -1.0,
                },
                Direction::Flat => 0.0,
            };

            // Base weight proportional to confidence and volatility scaling, clamped by position limit
            let base_weight = (dir_mult * sig.confidence * vol_scalar)
                .clamp(-constraints.max_position_pct, constraints.max_position_pct);

            // Apply drawdown de-risking factor
            let final_pos_weight = base_weight * derisk_scalar;
            raw_weights.push((sig.instrument, sig.symbol.clone(), final_pos_weight));
        }

        // 2. Dollar-neutral adjustment if requested
        if constraints.long_short_mode == LongShortMode::DollarNeutral && !raw_weights.is_empty() {
            let active_count = raw_weights.iter().filter(|w| w.2.abs() > 1e-8).count();
            if active_count > 0 {
                let sum_weights: f64 = raw_weights.iter().map(|w| w.2).sum();
                let mean_offset = sum_weights / active_count as f64;
                for w in &mut raw_weights {
                    if w.2.abs() > 1e-8 {
                        w.2 -= mean_offset;
                    }
                }
            }
        }

        // 3. Gross exposure scaling: sum(|weights|) <= max_gross_exposure
        let total_gross: f64 = raw_weights.iter().map(|w| w.2.abs()).sum();
        let scale_factor = if total_gross > constraints.max_gross_exposure && total_gross > 1e-8 {
            constraints.max_gross_exposure / total_gross
        } else {
            1.0
        };

        // 4. Construct TargetPosition records
        for (inst, sym, weight) in raw_weights {
            let final_weight = weight * scale_factor;
            let target_value = final_weight * nav;
            let price = prices.get(&inst).copied().unwrap_or(1.0).max(1e-4);
            let target_quantity = (target_value / price).round();

            target_positions.add(TargetPosition {
                instrument: inst,
                symbol: sym,
                target_weight: final_weight,
                target_value,
                target_quantity,
            });
        }

        target_positions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_signal(id: u32, dir: Direction, conf: f64) -> Signal {
        Signal {
            direction: dir,
            expected_return: if dir == Direction::Long { 0.02 } else { -0.02 },
            confidence: conf,
            horizon_bars: 1,
            instrument: InstrumentId(id),
            symbol: format!("S{}", id),
            model_id: "lstm_v1".to_string(),
            as_of: 1000,
        }
    }

    #[test]
    fn test_volatility_targeted_constructor_long_only() {
        let constructor = VolatilityTargetedConstructor::new(0.20);
        let portfolio = Portfolio::new(100_000.0);
        let mut prices = HashMap::new();
        prices.insert(InstrumentId(1), 100.0);
        prices.insert(InstrumentId(2), 50.0);

        let signals = vec![
            make_test_signal(1, Direction::Long, 0.8),
            make_test_signal(2, Direction::Short, 0.8),
        ];

        let constraints = PortfolioConstraints {
            max_gross_exposure: 1.0,
            max_position_pct: 0.25,
            volatility_target: Some(0.15),
            long_short_mode: LongShortMode::LongOnly,
            ..Default::default()
        };

        let targets = constructor.target_positions(&signals, &portfolio, &prices, &constraints);
        let t1 = targets.get(&InstrumentId(1)).unwrap();
        let t2 = targets.get(&InstrumentId(2)).unwrap();

        assert!(t1.target_weight > 0.0);
        assert_eq!(t2.target_weight, 0.0); // Filtered out by LongOnly
        assert!(targets.gross_weight <= constraints.max_gross_exposure);
    }

    #[test]
    fn test_drawdown_derisking_multiplier_applied() {
        let constructor = VolatilityTargetedConstructor::new(0.20);
        let mut portfolio = Portfolio::new(100_000.0);
        portfolio.peak_nav = 120_000.0; // Drawdown = (120k - 100k)/120k = 16.6% (> 10% threshold)

        let mut prices = HashMap::new();
        prices.insert(InstrumentId(1), 100.0);

        let signals = vec![make_test_signal(1, Direction::Long, 1.0)];
        let constraints = PortfolioConstraints {
            drawdown_derisk_threshold: 0.10,
            drawdown_derisk_multiplier: 0.50,
            ..Default::default()
        };

        let targets = constructor.target_positions(&signals, &portfolio, &prices, &constraints);
        let t1 = targets.get(&InstrumentId(1)).unwrap();

        // Target weight should reflect the 0.50 derisking multiplier
        assert!(t1.target_weight <= constraints.max_position_pct * 0.50 + 1e-4);
    }
}
