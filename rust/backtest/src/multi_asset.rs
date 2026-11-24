//! Synchronized multi-asset replay with one cash account and explicit alignment.
use crate::{BacktestEngine, BacktestError, BacktestReport, SignalStream};
use quant_data::Bar;
use quant_execution::{ExecutionModel, Fill, Order};
use quant_instruments::InstrumentId;
use quant_portfolio::{LongShortMode, Portfolio, PortfolioConstraints, PortfolioConstructor};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};

pub struct AssetBars<'a> {
    pub instrument: InstrumentId,
    pub symbol: &'a str,
    pub bars: &'a [Bar],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetPosition {
    pub timestamp: i64,
    pub instrument: InstrumentId,
    pub symbol: String,
    pub quantity: f64,
    pub market_value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultiAssetReport {
    pub report: BacktestReport,
    pub positions: Vec<AssetPosition>,
    pub cash: Vec<(i64, f64)>,
}

impl BacktestEngine {
    /// All series must use the same observed and availability timestamps. No
    /// forward filling, calendar intersection or hidden price substitution occurs.
    /// Signals must precede the execution bar's observed timestamp. Fills use the
    /// bar-close simulator, as in single-asset replay, and sales precede purchases.
    pub fn run_multi_asset<
        S: SignalStream,
        C: PortfolioConstructor + ?Sized,
        E: ExecutionModel + ?Sized,
    >(
        &self,
        mut stream: S,
        constructor: &C,
        execution: &E,
        assets: &[AssetBars<'_>],
        constraints: &PortfolioConstraints,
    ) -> Result<MultiAssetReport, BacktestError> {
        let invalid = |message: &str| BacktestError::InvalidData(message.into());
        if assets.is_empty() || assets[0].bars.is_empty() {
            return Err(BacktestError::EmptyData);
        }
        if !self.config.initial_cash.is_finite()
            || self.config.initial_cash <= 0.0
            || !self.config.periods_per_year.is_finite()
            || self.config.periods_per_year <= 0.0
            || !self.config.risk_free_rate.is_finite()
        {
            return Err(invalid("Invalid capital or annualization"));
        }
        if [
            constraints.max_gross_exposure,
            constraints.max_net_exposure,
            constraints.max_position_pct,
            constraints.drawdown_derisk_threshold,
            constraints.drawdown_derisk_multiplier,
        ]
        .iter()
        .any(|x| !x.is_finite() || *x < 0.0)
            || constraints
                .volatility_target
                .is_some_and(|x| !x.is_finite() || x < 0.0)
        {
            return Err(invalid("Invalid portfolio constraints"));
        }
        let mut ordered: Vec<_> = assets.iter().collect();
        ordered.sort_by_key(|a| a.instrument.0);
        let mut ids = BTreeSet::new();
        let mut symbols = BTreeSet::new();
        let timeline = ordered[0].bars;
        for asset in &ordered {
            if !ids.insert(asset.instrument.0) || !symbols.insert(asset.symbol) {
                return Err(invalid("Duplicate instrument or symbol"));
            }
            quant_data::validate_bars_monotonic_and_sound(asset.bars)
                .map_err(|e| BacktestError::InvalidData(e.to_string()))?;
            if asset.bars.len() != timeline.len()
                || asset.bars.iter().zip(timeline).any(|(a, b)| {
                    a.timestamp != b.timestamp
                        || a.availability_timestamp != b.availability_timestamp
                        || a.availability_timestamp < a.timestamp
                })
                || asset
                    .bars
                    .windows(2)
                    .any(|w| w[0].availability_timestamp >= w[1].timestamp)
            {
                return Err(invalid("Multi-asset bars must have identical, strictly increasing observation and availability times"));
            }
        }
        let mut portfolio = Portfolio::new(self.config.initial_cash);
        let initial_time = timeline[0]
            .availability_timestamp
            .as_nanos()
            .saturating_sub(1);
        let mut equity = vec![(initial_time, self.config.initial_cash)];
        let mut benchmark = equity.clone();
        let mut benchmark_returns = Vec::new();
        let mut fills = Vec::new();
        let mut positions = Vec::new();
        let mut cash = Vec::new();
        let mut prices = HashMap::new();
        let mut benchmark_previous = self.config.initial_cash;
        for (index, bar) in timeline.iter().enumerate() {
            let observed = bar.timestamp.as_nanos();
            let available = bar.availability_timestamp.as_nanos();
            for asset in &ordered {
                // Size orders only with prices available before this bar begins.
                prices.insert(asset.instrument, asset.bars[index.saturating_sub(1)].close);
            }
            portfolio.update_market_prices(&prices);
            let batch = if index == 0 {
                Vec::new()
            } else {
                stream.next_batch(observed.saturating_sub(1))
            };
            let mut signals = HashMap::new();
            for signal in batch {
                let asset = ordered
                    .iter()
                    .find(|a| a.instrument == signal.instrument)
                    .ok_or_else(|| invalid("Signal refers to an unknown instrument"))?;
                if signal.symbol != asset.symbol
                    || signal.as_of >= observed
                    || !signal.expected_return.is_finite()
                    || !signal.confidence.is_finite()
                    || !(0.0..=1.0).contains(&signal.confidence)
                {
                    return Err(invalid("Invalid or noncausal signal"));
                }
                if signal.as_of < timeline[index - 1].availability_timestamp.as_nanos() {
                    continue;
                }
                if signals.insert(signal.instrument, signal).is_some() {
                    return Err(invalid(
                        "Multiple signals for an instrument in one execution interval",
                    ));
                }
            }
            let signals: Vec<_> = ordered
                .iter()
                .filter_map(|a| signals.remove(&a.instrument))
                .collect();
            let targets = constructor.target_positions(&signals, &portfolio, &prices, constraints);
            if targets.targets.keys().any(|id| !ids.contains(&id.0)) {
                return Err(invalid("Target refers to an unknown instrument"));
            }
            let mut orders = Vec::new();
            for asset in &ordered {
                let current = portfolio
                    .positions
                    .get(&asset.instrument)
                    .map_or(0.0, |p| p.quantity);
                let target = targets
                    .get(&asset.instrument)
                    .map_or(0.0, |p| p.target_quantity);
                if !target.is_finite()
                    || (constraints.long_short_mode == LongShortMode::LongOnly && target < 0.0)
                {
                    return Err(invalid("Invalid target position"));
                }
                if (target - current).abs() >= 1.0 {
                    orders.push((
                        asset,
                        Order::market(asset.instrument, asset.symbol, target - current, available),
                    ));
                }
            }
            orders.sort_by_key(|(a, o)| (o.quantity > 0.0, a.instrument.0));
            for (asset, mut order) in orders {
                let mut fill = execution.simulate_fill(&order, &asset.bars[index]);
                validate_fill(&fill, &order)?;
                // Never finance long-only fees/purchases with an implicit cash loan.
                if constraints.long_short_mode == LongShortMode::LongOnly
                    && fill.fill_quantity > 0.0
                {
                    for _ in 0..3 {
                        let spend = fill.fill_price * fill.fill_quantity + fill.commission;
                        if spend <= portfolio.cash {
                            break;
                        }
                        order.quantity = (order.quantity * portfolio.cash.max(0.0) / spend).floor();
                        fill = execution.simulate_fill(&order, &asset.bars[index]);
                        validate_fill(&fill, &order)?;
                    }
                    if fill.fill_price * fill.fill_quantity + fill.commission > portfolio.cash {
                        continue;
                    }
                }
                if fill.is_filled() {
                    portfolio.apply_trade(
                        fill.instrument,
                        &fill.symbol,
                        fill.fill_price,
                        fill.fill_quantity,
                        fill.commission,
                    );
                    fills.push(fill);
                }
            }
            for asset in &ordered {
                prices.insert(asset.instrument, asset.bars[index].close);
            }
            portfolio.update_market_prices(&prices);
            equity.push((available, portfolio.nav()));
            cash.push((available, portfolio.cash));
            for asset in &ordered {
                let quantity = portfolio
                    .positions
                    .get(&asset.instrument)
                    .map_or(0.0, |p| p.quantity);
                positions.push(AssetPosition {
                    timestamp: available,
                    instrument: asset.instrument,
                    symbol: asset.symbol.into(),
                    quantity,
                    market_value: quantity * prices[&asset.instrument],
                });
            }
            let nav = ordered
                .iter()
                .map(|a| {
                    self.config.initial_cash / ordered.len() as f64 * a.bars[index].close
                        / a.bars[0].close
                })
                .sum::<f64>();
            benchmark.push((available, nav));
            benchmark_returns.push(nav / benchmark_previous - 1.0);
            benchmark_previous = nav;
        }
        let benchmark_return = benchmark_previous / self.config.initial_cash - 1.0;
        let report = BacktestReport::compute_with_periods(
            self.config.initial_cash,
            equity,
            fills,
            self.config.risk_free_rate,
            self.config.num_prior_trials,
            self.config.periods_per_year,
        )
        .with_benchmark_and_positions(
            benchmark,
            benchmark_returns,
            benchmark_return,
            Vec::new(),
        );
        Ok(MultiAssetReport {
            report,
            positions,
            cash,
        })
    }
}

fn validate_fill(fill: &Fill, order: &Order) -> Result<(), BacktestError> {
    if fill.instrument != order.instrument
        || fill.symbol != order.symbol
        || fill.as_of != order.as_of
        || !fill.fill_price.is_finite()
        || fill.fill_price <= 0.0
        || !fill.fill_quantity.is_finite()
        || !fill.commission.is_finite()
        || !fill.slippage.is_finite()
        || fill.fill_quantity.abs() > order.quantity.abs()
        || fill.fill_quantity * order.quantity < 0.0
        || !(fill.fill_price * fill.fill_quantity + fill.commission).is_finite()
    {
        return Err(BacktestError::ExecutionFailed(
            "Invalid simulated fill".into(),
        ));
    }
    Ok(())
}
