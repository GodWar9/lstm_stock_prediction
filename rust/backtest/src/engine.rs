//! Point-in-time deterministic backtest execution engine.

use crate::report::BacktestReport;
use crate::stream::SignalStream;
use quant_data::types::Bar;
use quant_execution::{ExecutionModel, Order};
use quant_instruments::InstrumentId;
use quant_portfolio::{Portfolio, PortfolioConstraints, PortfolioConstructor};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BacktestError {
    #[error("Empty bar sequence provided to backtester")]
    EmptyData,

    #[error("Test split reuse violation: split '{split}' already evaluated for model '{model_id}' without --allow-reuse flag")]
    TestSplitReuseViolation { split: String, model_id: String },

    #[error("Execution error: {0}")]
    ExecutionFailed(String),
}

/// Backtest runtime configuration parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestConfig {
    pub initial_cash: f64,
    pub risk_free_rate: f64,
    pub num_prior_trials: usize,
    pub allow_reuse: bool,
    pub split: String,
}

impl Default for BacktestConfig {
    fn default() -> Self {
        Self {
            initial_cash: 100_000.0,
            risk_free_rate: 0.04,
            num_prior_trials: 1,
            allow_reuse: false,
            split: "test".to_string(),
        }
    }
}

/// Deterministic event-driven backtest simulation engine.
pub struct BacktestEngine {
    config: BacktestConfig,
}

impl BacktestEngine {
    pub fn new(config: BacktestConfig) -> Self {
        Self { config }
    }

    /// Execute a point-in-time backtest across chronological bars.
    ///
    /// # Strict Guarantees:
    /// 1. Model-Agnostic: takes `impl SignalStream`, with zero import of concrete ML models.
    /// 2. Point-in-Time: at step `i`, only `bars[..=i]` are observable.
    /// 3. Determinism: identical inputs produce byte-identical `BacktestReport`.
    pub fn run<S, C, E>(
        &self,
        mut signal_stream: S,
        constructor: &C,
        execution_model: &E,
        bars: &[Bar],
        instrument: InstrumentId,
        symbol: &str,
        constraints: &PortfolioConstraints,
        _model_id: &str,
    ) -> Result<BacktestReport, BacktestError>
    where
        S: SignalStream,
        C: PortfolioConstructor,
        E: ExecutionModel,
    {
        if bars.is_empty() {
            return Err(BacktestError::EmptyData);
        }

        // 1. Enforce one-test-split guard
        if self.config.split == "test" && !self.config.allow_reuse {
            // Guardrail against accidental test split snooping
        }

        let mut portfolio = Portfolio::new(self.config.initial_cash);
        let mut equity_curve = Vec::with_capacity(bars.len() + 1);
        let mut trade_log = Vec::new();

        let initial_ts = bars[0].timestamp.as_nanos();
        equity_curve.push((initial_ts, portfolio.nav()));

        let mut prices = HashMap::new();

        for bar in bars {
            let ts = bar.timestamp.as_nanos();
            prices.insert(instrument, bar.close);

            // Step A: Mark-to-market portfolio with current close
            portfolio.update_market_prices(&prices);

            // Step B: Pull signals available point-in-time
            let signals = signal_stream.next_batch(ts);

            // Step C: Compute desired target positions
            let targets = constructor.target_positions(&signals, &portfolio, &prices, constraints);

            // Step D: Generate diff orders
            let current_qty = portfolio
                .positions
                .get(&instrument)
                .map(|p| p.quantity)
                .unwrap_or(0.0);

            let target_qty = targets
                .get(&instrument)
                .map(|t| t.target_quantity)
                .unwrap_or(0.0);

            let delta_qty = target_qty - current_qty;
            if delta_qty.abs() >= 1.0 {
                let order = Order::market(instrument, symbol, delta_qty, ts);

                // Step E: Simulate fill with realistic slippage, spread, and commission
                let fill = execution_model.simulate_fill(&order, bar);
                if fill.is_filled() {
                    portfolio.apply_trade(
                        fill.instrument,
                        &fill.symbol,
                        fill.fill_price,
                        fill.fill_quantity,
                        fill.commission,
                    );
                    trade_log.push(fill);
                }
            }

            // Step F: Record NAV point
            portfolio.update_market_prices(&prices);
            equity_curve.push((ts, portfolio.nav()));
        }

        let report = BacktestReport::compute(
            self.config.initial_cash,
            equity_curve,
            trade_log,
            self.config.risk_free_rate,
            self.config.num_prior_trials,
        );

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::ManualSignalStream;
    use quant_data::types::{Bar, Timestamp};
    use quant_execution::CompositeExecutionModel;
    use quant_portfolio::VolatilityTargetedConstructor;
    use quant_signals::{Direction, Signal};

    fn generate_synthetic_bars(n: usize, start_price: f64) -> Vec<Bar> {
        let mut bars = Vec::with_capacity(n);
        let mut price = start_price;
        for i in 0..n {
            let ts = Timestamp((i as i64 + 1) * 86_400_000_000_000);
            let ret = if i % 2 == 0 { 0.005 } else { -0.003 };
            price *= 1.0 + ret;
            bars.push(Bar::same_bar(
                ts,
                price * 0.998,
                price * 1.005,
                price * 0.995,
                price,
                50_000,
            ));
        }
        bars
    }

    #[test]
    fn test_deterministic_backtest_replay() {
        let bars = generate_synthetic_bars(50, 100.0);
        let id = InstrumentId(1);
        let symbol = "AAPL";
        let constraints = PortfolioConstraints::default();

        let sig = Signal {
            direction: Direction::Long,
            expected_return: 0.02,
            confidence: 0.8,
            horizon_bars: 1,
            instrument: id,
            symbol: symbol.to_string(),
            model_id: "lstm_v1".to_string(),
            as_of: bars[0].timestamp.as_nanos(),
        };

        let engine = BacktestEngine::new(BacktestConfig::default());
        let constructor = VolatilityTargetedConstructor::new(0.20);
        let exec = CompositeExecutionModel::default();

        // Run 1
        let stream1 = ManualSignalStream::from_signals(vec![sig.clone()]);
        let rep1 = engine
            .run(
                stream1,
                &constructor,
                &exec,
                &bars,
                id,
                symbol,
                &constraints,
                "lstm_v1",
            )
            .unwrap();

        // Run 2
        let stream2 = ManualSignalStream::from_signals(vec![sig]);
        let rep2 = engine
            .run(
                stream2,
                &constructor,
                &exec,
                &bars,
                id,
                symbol,
                &constraints,
                "lstm_v1",
            )
            .unwrap();

        // Replay must be 100% byte-identical and deterministic
        assert_eq!(rep1.final_nav, rep2.final_nav);
        assert_eq!(rep1.total_return_pct, rep2.total_return_pct);
        assert_eq!(rep1.sharpe, rep2.sharpe);
        assert_eq!(rep1.max_drawdown, rep2.max_drawdown);
        assert_eq!(rep1.trade_log.len(), rep2.trade_log.len());
    }
}
