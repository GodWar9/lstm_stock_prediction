//! Backtest report structures and statistical performance metrics.

use quant_execution::Fill;
use serde::{Deserialize, Serialize};

/// Comprehensive point-in-time backtest performance report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BacktestReport {
    /// Time series of (timestamp_nanos/millis, NAV) points.
    pub equity_curve: Vec<(i64, f64)>,
    /// Daily/bar return series.
    pub returns: Vec<f64>,
    /// Initial capital.
    pub initial_cash: f64,
    /// Final portfolio Net Asset Value.
    pub final_nav: f64,
    /// Total cumulative strategy return: (final_nav - initial_cash) / initial_cash.
    pub total_return_pct: f64,
    /// Annualized Compound Annual Growth Rate (CAGR).
    pub cagr: f64,
    /// Annualized Sharpe Ratio (risk-free rate subtracted).
    pub sharpe: f64,
    /// Annualized Sortino Ratio (downside risk penalization).
    pub sortino: f64,
    /// Calmar Ratio (CAGR / Max Drawdown).
    pub calmar: f64,
    /// Maximum peak-to-trough drawdown fraction.
    pub max_drawdown: f64,
    /// Profit factor: gross profits / gross losses.
    pub profit_factor: f64,
    /// Total portfolio turnover rate.
    pub turnover: f64,
    /// Percentage of winning trades: winning_trades / total_trades.
    pub hit_rate: f64,
    /// Average return on winning trades.
    pub avg_win: f64,
    /// Average return on losing trades.
    pub avg_loss: f64,
    /// Total number of executed trades.
    pub total_trades: usize,
    /// Number of profitable trades.
    pub winning_trades: usize,
    /// Number of unprofitable trades.
    pub losing_trades: usize,
    /// Deflated Sharpe Ratio estimate adjusting for multiple trial selection bias.
    pub deflated_sharpe: f64,
    /// Detailed audit log of every simulated fill.
    pub trade_log: Vec<Fill>,
    /// Position snapshots: (timestamp_nanos, quantity, market_value).
    #[serde(default)]
    pub positions_curve: Vec<(i64, f64, f64)>,
    /// Benchmark equity curve: (timestamp_nanos, nav).
    #[serde(default)]
    pub benchmark_curve: Vec<(i64, f64)>,
    /// Benchmark period returns.
    #[serde(default)]
    pub benchmark_returns: Vec<f64>,
    /// Benchmark total return.
    #[serde(default)]
    pub benchmark_total_return: f64,
}

impl BacktestReport {
    /// Compute full backtest report from equity curve and fill log.
    pub fn compute(
        initial_cash: f64,
        equity_curve: Vec<(i64, f64)>,
        trade_log: Vec<Fill>,
        risk_free_rate: f64,
        num_prior_trials: usize,
    ) -> Self {
        let final_nav = equity_curve.last().map(|p| p.1).unwrap_or(initial_cash);
        let total_return_pct = if initial_cash > 1e-8 {
            (final_nav - initial_cash) / initial_cash
        } else {
            0.0
        };

        // Compute bar returns
        let mut returns = Vec::with_capacity(equity_curve.len().saturating_sub(1));
        for i in 1..equity_curve.len() {
            let prev = equity_curve[i - 1].1;
            let curr = equity_curve[i].1;
            if prev > 1e-8 {
                returns.push((curr - prev) / prev);
            } else {
                returns.push(0.0);
            }
        }

        let n = returns.len() as f64;
        let cagr = if n >= 2.0 && initial_cash > 1e-8 && final_nav > 0.0 {
            let years = n / 252.0;
            if years > 0.05 {
                (final_nav / initial_cash).powf(1.0 / years) - 1.0
            } else {
                total_return_pct
            }
        } else {
            total_return_pct
        };

        // Max drawdown calculation
        let mut peak = initial_cash;
        let mut max_dd = 0.0;
        for &(_, nav) in &equity_curve {
            if nav > peak {
                peak = nav;
            }
            if peak > 1e-8 {
                let dd = (peak - nav) / peak;
                if dd > max_dd {
                    max_dd = dd;
                }
            }
        }

        // Sharpe & Sortino ratios
        let daily_rf = risk_free_rate / 252.0;
        let (sharpe, sortino) = if n >= 2.0 {
            let mean = returns.iter().sum::<f64>() / n;
            let variance = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (n - 1.0);
            let vol = variance.sqrt();

            let downside_var = returns
                .iter()
                .map(|r| {
                    if *r < daily_rf {
                        (r - daily_rf).powi(2)
                    } else {
                        0.0
                    }
                })
                .sum::<f64>()
                / (n - 1.0);
            let downside_vol = downside_var.sqrt();

            let annualized_sharpe = if vol > 1e-8 {
                ((mean - daily_rf) / vol) * (252.0_f64).sqrt()
            } else {
                0.0
            };

            let annualized_sortino = if downside_vol > 1e-8 {
                ((mean - daily_rf) / downside_vol) * (252.0_f64).sqrt()
            } else {
                0.0
            };

            (annualized_sharpe, annualized_sortino)
        } else {
            (0.0, 0.0)
        };

        let calmar = if max_dd > 1e-4 { cagr / max_dd } else { 0.0 };

        // Trade performance stats from fills
        let total_trades = trade_log.len();
        let mut winning_trades = 0;
        let mut losing_trades = 0;
        let mut gross_profits = 0.0;
        let mut gross_losses = 0.0;
        let mut win_returns = Vec::new();
        let mut loss_returns = Vec::new();

        // Average-cost realized PnL, with entry fees allocated proportionally on close.
        let mut holdings: std::collections::HashMap<_, (f64, f64, f64)> =
            std::collections::HashMap::new();
        let mut closed_trades = 0usize;
        for f in &trade_log {
            let state = holdings.entry(f.instrument).or_insert((0.0, 0.0, 0.0));
            let (qty, price, fees) = *state;
            if qty.abs() < 1e-8 || qty.signum() == f.fill_quantity.signum() {
                let next = qty + f.fill_quantity;
                if next.abs() > 1e-8 {
                    *state = (
                        next,
                        (qty.abs() * price + f.fill_quantity.abs() * f.fill_price) / next.abs(),
                        fees + f.commission,
                    );
                }
                continue;
            }
            let closed = qty.abs().min(f.fill_quantity.abs());
            let exit_fee = f.commission * closed / f.fill_quantity.abs();
            let entry_fee = fees * closed / qty.abs();
            let pnl = closed * qty.signum() * (f.fill_price - price) - entry_fee - exit_fee;
            closed_trades += 1;
            if pnl > 0.0 {
                winning_trades += 1;
                gross_profits += pnl;
                win_returns.push(pnl / (closed * price));
            } else if pnl < 0.0 {
                losing_trades += 1;
                gross_losses -= pnl;
                loss_returns.push(pnl / (closed * price));
            }
            let next = qty + f.fill_quantity;
            *state = if next.abs() < 1e-8 {
                (0.0, 0.0, 0.0)
            } else if next.signum() == qty.signum() {
                (next, price, fees - entry_fee)
            } else {
                (next, f.fill_price, f.commission - exit_fee)
            };
        }
        let turnover = if initial_cash > 0.0 {
            trade_log.iter().map(|f| f.notional()).sum::<f64>() / initial_cash
        } else {
            0.0
        };
        let hit_rate = if closed_trades > 0 {
            winning_trades as f64 / closed_trades as f64
        } else {
            0.0
        };

        let profit_factor = if gross_losses > 1e-8 {
            gross_profits / gross_losses
        } else if gross_profits > 1e-8 {
            10.0
        } else {
            1.0
        };

        let avg_win = if !win_returns.is_empty() {
            win_returns.iter().sum::<f64>() / win_returns.len() as f64
        } else {
            0.0
        };

        let avg_loss = if !loss_returns.is_empty() {
            loss_returns.iter().sum::<f64>() / loss_returns.len() as f64
        } else {
            0.0
        };

        // Deflated Sharpe estimate (Bailey & López de Prado): penalize for trials
        let trials_penalty = ((num_prior_trials.max(1) as f64).ln()).sqrt() * 0.15;
        let deflated_sharpe = (sharpe - trials_penalty).max(-5.0);

        Self {
            equity_curve,
            returns,
            initial_cash,
            final_nav,
            total_return_pct,
            cagr,
            sharpe,
            sortino,
            calmar,
            max_drawdown: max_dd,
            profit_factor,
            turnover,
            hit_rate,
            avg_win,
            avg_loss,
            total_trades,
            winning_trades,
            losing_trades,
            deflated_sharpe,
            trade_log,
            positions_curve: Vec::new(),
            benchmark_curve: Vec::new(),
            benchmark_returns: Vec::new(),
            benchmark_total_return: 0.0,
        }
    }

    /// Attach benchmark and position histories to the computed report.
    pub fn with_benchmark_and_positions(
        mut self,
        benchmark_curve: Vec<(i64, f64)>,
        benchmark_returns: Vec<f64>,
        benchmark_total_return: f64,
        positions_curve: Vec<(i64, f64, f64)>,
    ) -> Self {
        self.benchmark_curve = benchmark_curve;
        self.benchmark_returns = benchmark_returns;
        self.benchmark_total_return = benchmark_total_return;
        self.positions_curve = positions_curve;
        self
    }
}
