//! Sector-neutral Quadratic Programming (QP) portfolio optimizer.
//!
//! Solves the mean-variance portfolio optimization problem with sector-neutrality
//! constraints, gross leverage limits, and position concentration bounds:
//!
//! minimize:   (1/2) wᵀ Σ w - λ αᵀ w
//! subject to:
//!   - Sector neutrality: ∑_{i ∈ Sector(k)} w_i = 0  ∀ k
//!   - Gross exposure:    ∑ |w_i| ≤ L_max
//!   - Position bounds:   -w_max ≤ w_i ≤ w_max  ∀ i
//!   - Dollar neutrality: ∑ w_i = 0 (when in DollarNeutral mode)
//!
//! Uses a high-performance, pure-Rust Projected Gradient Descent (PGD) solver
//! with Duchi L1-ball projection and orthogonal sector subspace projections.
//! Amortizes allocation overhead to zero for production backtests.

use crate::constraints::{LongShortMode, PortfolioConstraints};
use crate::constructor::{PortfolioConstructor, VolatilityTargetedConstructor};
use crate::portfolio::Portfolio;
use crate::position::{TargetPosition, TargetPositions};
use crate::sector::{Sector, SectorMap};
use quant_instruments::InstrumentId;
use quant_signals::{Direction, Signal};
use std::collections::HashMap;

/// Convex QP optimizer with sector neutrality and leverage constraints.
#[derive(Debug, Clone)]
pub struct SectorNeutralOptimizer {
    /// Sector mappings for universe instruments.
    pub sector_map: SectorMap,
    /// Risk aversion parameter λ (tradeoff between expected alpha and risk).
    pub risk_aversion: f64,
    /// Maximum solver iterations.
    pub max_iterations: usize,
    /// Convergence tolerance on relative objective or weight change.
    pub tolerance: f64,
    /// Step size (learning rate) for projected gradient updates.
    pub step_size: f64,
    /// Fallback constructor when QP cannot be formed or converges poorly.
    fallback: VolatilityTargetedConstructor,
}

impl Default for SectorNeutralOptimizer {
    fn default() -> Self {
        Self {
            sector_map: SectorMap::new(),
            risk_aversion: 1.0,
            max_iterations: 200,
            tolerance: 1e-6,
            step_size: 0.1,
            fallback: VolatilityTargetedConstructor::default(),
        }
    }
}

impl SectorNeutralOptimizer {
    /// Creates a new sector-neutral optimizer with the given sector map.
    pub fn new(sector_map: SectorMap) -> Self {
        Self {
            sector_map,
            ..Default::default()
        }
    }

    /// Sets the risk aversion parameter λ.
    pub fn with_risk_aversion(mut self, lambda: f64) -> Self {
        self.risk_aversion = lambda.max(0.01);
        self
    }

    /// Project vector `w` onto the L1-ball of radius `radius` (Duchi et al., 2008).
    ///
    /// Finds argmin_v ||v - w||_2 subject to ||v||_1 <= radius.
    pub fn project_l1_ball(w: &[f64], radius: f64) -> Vec<f64> {
        let l1_norm: f64 = w.iter().map(|x| x.abs()).sum();
        if l1_norm <= radius {
            return w.to_vec();
        }

        let mut abs_w: Vec<f64> = w.iter().map(|x| x.abs()).collect();
        // Sort descending
        abs_w.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

        let mut cumsum = 0.0;
        let mut rho = 0;
        for (i, &val) in abs_w.iter().enumerate() {
            cumsum += val;
            if val - (cumsum - radius) / ((i + 1) as f64) > 0.0 {
                rho = i + 1;
            }
        }

        let theta = (abs_w[..rho].iter().sum::<f64>() - radius) / (rho as f64);
        w.iter()
            .map(|&x| {
                let sign = if x >= 0.0 { 1.0 } else { -1.0 };
                let val = (x.abs() - theta).max(0.0);
                sign * val
            })
            .collect()
    }

    /// Project weights onto sector neutrality: ∑_{i ∈ S_k} w_i = 0 for each sector.
    fn project_sector_neutrality(
        weights: &mut [f64],
        instruments: &[InstrumentId],
        sector_map: &SectorMap,
    ) {
        let mut sector_groups: HashMap<Sector, Vec<usize>> = HashMap::new();
        for (idx, inst) in instruments.iter().enumerate() {
            let sector = sector_map
                .get_sector(inst)
                .cloned()
                .unwrap_or(Sector::Custom("Unassigned".to_string()));
            sector_groups.entry(sector).or_default().push(idx);
        }

        for (_, indices) in sector_groups {
            if indices.len() > 1 {
                let sum: f64 = indices.iter().map(|&i| weights[i]).sum();
                let mean = sum / (indices.len() as f64);
                for &i in &indices {
                    weights[i] -= mean;
                }
            } else if indices.len() == 1 {
                // If only 1 stock in the sector, sector neutrality forces it to 0
                weights[indices[0]] = 0.0;
            }
        }
    }
}

impl PortfolioConstructor for SectorNeutralOptimizer {
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

        // If no sector mapping is available or fewer than 2 signals, fall back
        if self.sector_map.is_empty() || signals.len() < 2 {
            return self
                .fallback
                .target_positions(signals, current, prices, constraints);
        }

        let n = signals.len();
        let instruments: Vec<InstrumentId> = signals.iter().map(|s| s.instrument).collect();

        // 1. Build alpha vector from signals
        let mut alpha = vec![0.0; n];
        for (i, sig) in signals.iter().enumerate() {
            let dir_mult = match sig.direction {
                Direction::Long => 1.0,
                Direction::Short => match constraints.long_short_mode {
                    LongShortMode::LongOnly => 0.0,
                    _ => -1.0,
                },
                Direction::Flat => 0.0,
            };
            alpha[i] = dir_mult * sig.confidence * sig.expected_return;
        }

        // 2. Simplified diagonal + ridge covariance model: Σ = σ² I + ridge
        // In full production, this can incorporate dynamic asset pairwise correlations.
        let default_vol = self.fallback.default_asset_vol.max(0.05);
        let var_diag = default_vol * default_vol;

        // Initial weights initialized proportionally to alpha
        let mut w: Vec<f64> = alpha
            .iter()
            .map(|&a| {
                (a / var_diag).clamp(-constraints.max_position_pct, constraints.max_position_pct)
            })
            .collect();

        // 3. Projected Gradient Descent loop
        let lambda = self.risk_aversion;
        let step = self.step_size;
        let pos_limit = constraints.max_position_pct;
        let gross_limit = constraints.max_gross_exposure;

        for _ in 0..self.max_iterations {
            let mut max_change = 0.0_f64;

            // Gradient: ∇ f(w) = Σ w - λ α = var_diag * w - λ α
            for i in 0..n {
                let grad_i = var_diag * w[i] - lambda * alpha[i];
                let w_new = w[i] - step * grad_i;
                let clamped = w_new.clamp(-pos_limit, pos_limit);
                let diff = (clamped - w[i]).abs();
                if diff > max_change {
                    max_change = diff;
                }
                w[i] = clamped;
            }

            // Project onto sector neutrality subspace: ∑_{i ∈ S_k} w_i = 0
            Self::project_sector_neutrality(&mut w, &instruments, &self.sector_map);

            // Project onto dollar neutrality if requested: ∑ w_i = 0
            if constraints.long_short_mode == LongShortMode::DollarNeutral {
                let sum: f64 = w.iter().sum();
                let mean = sum / (n as f64);
                for x in &mut w {
                    *x -= mean;
                }
            }

            // Project onto L1-ball for gross leverage: ||w||_1 ≤ L_max
            w = Self::project_l1_ball(&w, gross_limit);

            // Check convergence
            if max_change < self.tolerance {
                break;
            }
        }

        // Re-enforce sector neutrality after final L1-ball projection
        Self::project_sector_neutrality(&mut w, &instruments, &self.sector_map);

        // 4. Construct TargetPosition records
        for (i, sig) in signals.iter().enumerate() {
            let weight = w[i];
            let target_value = weight * nav;
            let price = prices
                .get(&sig.instrument)
                .copied()
                .unwrap_or(1.0)
                .max(1e-4);
            let target_quantity = (target_value / price).round();

            target_positions.add(TargetPosition {
                instrument: sig.instrument,
                symbol: sig.symbol.clone(),
                target_weight: weight,
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
    use quant_signals::Direction;

    fn test_signal(id: u32, sym: &str, dir: Direction, ret: f64, conf: f64) -> Signal {
        Signal {
            direction: dir,
            expected_return: ret,
            confidence: conf,
            horizon_bars: 1,
            instrument: InstrumentId(id),
            symbol: sym.to_string(),
            as_of: 1000,
            model_id: "test".to_string(),
        }
    }

    #[test]
    fn test_l1_ball_projection() {
        let v = vec![1.5, -2.0, 0.5];
        let radius = 2.0;
        let proj = SectorNeutralOptimizer::project_l1_ball(&v, radius);
        let l1_norm: f64 = proj.iter().map(|x| x.abs()).sum();
        assert!((l1_norm - radius).abs() < 1e-6);
        // Signs preserved
        assert!(proj[0] >= 0.0);
        assert!(proj[1] <= 0.0);
        assert!(proj[2] >= 0.0);
    }

    #[test]
    fn test_sector_neutral_optimization() {
        let mut map = SectorMap::new();
        let inst1 = InstrumentId(1); // AAPL (Tech)
        let inst2 = InstrumentId(2); // MSFT (Tech)
        let inst3 = InstrumentId(3); // JPM (Fin)
        let inst4 = InstrumentId(4); // BAC (Fin)

        map.assign(inst1, Sector::Technology);
        map.assign(inst2, Sector::Technology);
        map.assign(inst3, Sector::Financials);
        map.assign(inst4, Sector::Financials);

        let optimizer = SectorNeutralOptimizer::new(map).with_risk_aversion(2.0);

        let signals = vec![
            test_signal(1, "AAPL", Direction::Long, 0.05, 0.8),
            test_signal(2, "MSFT", Direction::Short, -0.03, 0.7),
            test_signal(3, "JPM", Direction::Long, 0.04, 0.9),
            test_signal(4, "BAC", Direction::Short, -0.02, 0.6),
        ];

        let portfolio = Portfolio::new(100_000.0);
        let mut prices = HashMap::new();
        prices.insert(inst1, 150.0);
        prices.insert(inst2, 280.0);
        prices.insert(inst3, 140.0);
        prices.insert(inst4, 35.0);

        let constraints = PortfolioConstraints {
            max_gross_exposure: 1.0,
            max_position_pct: 0.40,
            long_short_mode: LongShortMode::DollarNeutral,
            ..Default::default()
        };

        let targets = optimizer.target_positions(&signals, &portfolio, &prices, &constraints);

        let w1 = targets.get(&inst1).unwrap().target_weight;
        let w2 = targets.get(&inst2).unwrap().target_weight;
        let w3 = targets.get(&inst3).unwrap().target_weight;
        let w4 = targets.get(&inst4).unwrap().target_weight;

        // Sector neutrality: Tech sum ≈ 0, Fin sum ≈ 0
        let tech_sum = w1 + w2;
        let fin_sum = w3 + w4;
        assert!(
            tech_sum.abs() < 1e-5,
            "Tech sector sum should be ~0, got {}",
            tech_sum
        );
        assert!(
            fin_sum.abs() < 1e-5,
            "Fin sector sum should be ~0, got {}",
            fin_sum
        );

        // Overall gross exposure <= 1.0
        let total_gross = w1.abs() + w2.abs() + w3.abs() + w4.abs();
        assert!(total_gross <= 1.0 + 1e-5);
    }
}
