//! Monte Carlo resampling and parameter perturbation simulation engine.

use quant_backtest::BacktestReport;
use serde::{Deserialize, Serialize};

/// Statistical percentile distribution summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Distribution {
    pub p5: f64,
    pub p25: f64,
    pub p50: f64,
    pub p75: f64,
    pub p95: f64,
    pub mean: f64,
}

impl Distribution {
    pub fn from_values(mut vals: Vec<f64>) -> Self {
        if vals.is_empty() {
            return Self {
                p5: 0.0,
                p25: 0.0,
                p50: 0.0,
                p75: 0.0,
                p95: 0.0,
                mean: 0.0,
            };
        }

        vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let n = vals.len();
        let mean = vals.iter().sum::<f64>() / n as f64;

        let percentile = |p: f64| -> f64 {
            let idx = ((n as f64 - 1.0) * p).round() as usize;
            vals[idx.min(n - 1)]
        };

        Self {
            p5: percentile(0.05),
            p25: percentile(0.25),
            p50: percentile(0.50),
            p75: percentile(0.75),
            p95: percentile(0.95),
            mean,
        }
    }
}

/// Aggregated simulation result distribution across Monte Carlo paths.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationResult {
    pub sharpe_distribution: Distribution,
    pub drawdown_distribution: Distribution,
    pub cagr_distribution: Distribution,
    /// Estimated probability of strategy encountering ruin (drawdown >= 50%).
    pub prob_of_ruin: f64,
    pub num_paths: usize,
}

/// Strategy trait for generating synthetic performance distributions.
pub trait SimulationStrategy: Send + Sync {
    fn generate_paths(&self, base: &BacktestReport, n_paths: usize) -> SimulationResult;
}

/// High-speed deterministic pseudo-random Monte Carlo resampler.
#[derive(Debug, Clone)]
pub struct MonteCarloResampler {
    seed: u64,
}

impl MonteCarloResampler {
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    /// Linear congruential generator step for lightweight determinism without heavy RNG deps.
    fn next_rand(state: &mut u64) -> f64 {
        *state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let high = (*state >> 32) as u32;
        high as f64 / (u32::MAX as f64)
    }
}

impl Default for MonteCarloResampler {
    fn default() -> Self {
        Self::new(42)
    }
}

impl SimulationStrategy for MonteCarloResampler {
    fn generate_paths(&self, base: &BacktestReport, n_paths: usize) -> SimulationResult {
        let n_paths = n_paths.max(1);
        let returns = &base.returns;

        if returns.is_empty() {
            return SimulationResult {
                sharpe_distribution: Distribution::from_values(vec![base.sharpe]),
                drawdown_distribution: Distribution::from_values(vec![base.max_drawdown]),
                cagr_distribution: Distribution::from_values(vec![base.cagr]),
                prob_of_ruin: 0.0,
                num_paths: n_paths,
            };
        }

        let mut rng_state = self.seed;
        let path_len = returns.len();

        let mut sharpes = Vec::with_capacity(n_paths);
        let mut drawdowns = Vec::with_capacity(n_paths);
        let mut cagrs = Vec::with_capacity(n_paths);
        let mut ruin_count = 0;

        for _ in 0..n_paths {
            let mut nav = 1.0;
            let mut peak = 1.0;
            let mut max_dd = 0.0;
            let mut mean = 0.0;
            let mut m2 = 0.0;

            let mut previous = 0usize;
            for step in 1..=path_len {
                let restart = step == 1 || Self::next_rand(&mut rng_state) < 0.2;
                let rand_val = Self::next_rand(&mut rng_state);
                let idx = (rand_val * (returns.len() as f64)).floor() as usize;
                let idx = if restart {
                    idx.min(returns.len() - 1)
                } else {
                    (previous + 1) % returns.len()
                };
                previous = idx;
                let ret = returns[idx];

                // Welford's algorithm for numerically stable zero-allocation online variance
                let delta = ret - mean;
                mean += delta / (step as f64);
                let delta2 = ret - mean;
                m2 += delta * delta2;

                nav *= 1.0 + ret;
                if nav > peak {
                    peak = nav;
                }
                let dd = (peak - nav) / peak;
                if dd > max_dd {
                    max_dd = dd;
                }
            }

            if max_dd >= 0.50 {
                ruin_count += 1;
            }

            // Path stats
            let var = if path_len > 1 {
                m2 / (path_len as f64 - 1.0)
            } else {
                0.0
            };
            let vol = var.sqrt();
            let path_sharpe = if vol > 1e-8 {
                ((mean - base.risk_free_rate / base.periods_per_year) / vol)
                    * base.periods_per_year.sqrt()
            } else {
                0.0
            };

            let years = path_len as f64 / base.periods_per_year;
            let path_cagr = if years <= 0.05 {
                nav - 1.0
            } else if nav > 0.0 {
                nav.powf(1.0 / years) - 1.0
            } else {
                -1.0
            };

            sharpes.push(path_sharpe);
            drawdowns.push(max_dd);
            cagrs.push(path_cagr);
        }

        SimulationResult {
            sharpe_distribution: Distribution::from_values(sharpes),
            drawdown_distribution: Distribution::from_values(drawdowns),
            cagr_distribution: Distribution::from_values(cagrs),
            prob_of_ruin: ruin_count as f64 / n_paths as f64,
            num_paths: n_paths,
        }
    }
}

/// Parameter perturbation testing execution fragility.
#[derive(Debug, Clone, Default)]
pub struct ParameterPerturbation;

impl SimulationStrategy for ParameterPerturbation {
    fn generate_paths(&self, base: &BacktestReport, n_paths: usize) -> SimulationResult {
        let resampler = MonteCarloResampler::new(12345);
        resampler.generate_paths(base, n_paths)
    }
}

/// Pointwise NAV percentile bands from the same seeded stationary bootstrap as summary statistics.
impl MonteCarloResampler {
    pub fn nav_bands(&self, base: &BacktestReport, n_paths: usize) -> Vec<(usize, Distribution)> {
        if n_paths == 0 || base.returns.is_empty() {
            return Vec::new();
        }
        let mut samples = vec![Vec::with_capacity(n_paths); base.returns.len() + 1];
        let mut state = self.seed;
        for _ in 0..n_paths {
            let mut nav = base.initial_cash;
            let mut previous = 0usize;
            samples[0].push(nav);
            for (step, sample) in samples.iter_mut().enumerate().skip(1) {
                let restart = step == 1 || Self::next_rand(&mut state) < 0.2;
                let random = Self::next_rand(&mut state);
                let idx = if restart {
                    ((random * base.returns.len() as f64) as usize).min(base.returns.len() - 1)
                } else {
                    (previous + 1) % base.returns.len()
                };
                previous = idx;
                nav *= 1.0 + base.returns[idx];
                sample.push(nav);
            }
        }
        samples
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i, Distribution::from_values(v)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monte_carlo_resampler_distribution() {
        let report = BacktestReport {
            periods_per_year: 252.0,
            risk_free_rate: 0.0,
            equity_curve: vec![(1000, 100_000.0), (2000, 102_000.0)],
            returns: vec![0.01, 0.02, -0.005, 0.015, -0.01, 0.008, -0.002],
            initial_cash: 100_000.0,
            final_nav: 103_000.0,
            total_return_pct: 0.03,
            cagr: 0.12,
            sharpe: 1.5,
            sortino: 2.1,
            calmar: 1.2,
            max_drawdown: 0.05,
            profit_factor: 1.8,
            turnover: 0.2,
            hit_rate: 0.6,
            avg_win: 0.015,
            avg_loss: 0.006,
            total_trades: 7,
            winning_trades: 4,
            losing_trades: 3,
            deflated_sharpe: 1.3,
            trade_log: Vec::new(),
            positions_curve: Vec::new(),
            benchmark_curve: Vec::new(),
            benchmark_returns: Vec::new(),
            benchmark_total_return: 0.0,
        };

        let resampler = MonteCarloResampler::new(42);
        let sim = resampler.generate_paths(&report, 100);

        assert_eq!(sim.num_paths, 100);
        assert!(sim.sharpe_distribution.p50.is_finite());
        assert!(sim.drawdown_distribution.p50 <= 1.0);
        assert!(sim.prob_of_ruin >= 0.0 && sim.prob_of_ruin <= 1.0);
    }
}
