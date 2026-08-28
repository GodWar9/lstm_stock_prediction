# 06 — Backtest & Simulation Engine (Rust)

## Model-Agnostic Backtest Contract

The backtester consumes a `SignalStream`, never a model:

```rust
pub trait SignalStream {
    fn next_batch(&mut self, as_of: Timestamp) -> Vec<Signal>;
}

// LSTM predictions, XGBoost predictions, or a hand-written rule can all
// implement this identically:
pub struct ModelSignalStream<P: PredictionProvider> { provider: P, /* ... */ }
pub struct ManualSignalStream { rules: Vec<Box<dyn Fn(&MarketState) -> Vec<Signal>>> }
```

`backtest::run()` takes `impl SignalStream` — it has no import path to
`ml::models::lstm` at all, satisfying the "backtester doesn't know it's ML"
requirement structurally, not by convention.

## Backtest Loop

```
for each timestamp in replay_range:
    market_state = data_engine.state_at(timestamp)      # PIT-correct, doc 02
    signals      = signal_stream.next_batch(timestamp)
    targets      = portfolio.target_positions(signals, current_portfolio, constraints)
    orders       = diff(targets, current_portfolio)
    fills        = execution_model.simulate_fill(orders, market_state)
    current_portfolio.apply(fills)
    risk_report  = risk_engine.evaluate(current_portfolio, market_state)
    record(timestamp, fills, current_portfolio, risk_report)
```

- Deterministic replay: same inputs (dataset version, model artifact,
  execution model, config, seed) always produce byte-identical output — this
  is a golden-tested property (doc `07-PERFORMANCE-TESTING.md`).
- The loop never looks past `timestamp` for any input — reuses the same PIT
  guard machinery as training-dataset construction (doc `02-DATA-FEATURES.md`),
  so backtest leakage and training leakage are prevented by the same code path.

## Output

```rust
pub struct BacktestReport {
    pub equity_curve: TimeSeries<f64>,
    pub sharpe: f64,
    pub sortino: f64,
    pub calmar: f64,
    pub max_drawdown: f64,
    pub profit_factor: f64,
    pub turnover: f64,
    pub hit_rate: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub tail_loss_95: f64,
    pub factor_exposure_over_time: TimeSeries<HashMap<String, f64>>,
    pub trade_log: Vec<Fill>,
}
```

A model with low MSE but a flat/negative `BacktestReport` is explicitly
labeled unsuccessful in the CLI report output — the platform doesn't let a
good loss curve stand in for a good strategy.

## Simulation Engine (Separate From the Backtester)

```rust
pub trait SimulationStrategy {
    fn generate_paths(&self, base: &BacktestReport, n_paths: usize) -> Vec<SimulatedPath>;
}

pub struct MonteCarloResampler;      // bootstrap resampling of trade returns
pub struct ParameterPerturbation;    // jitter execution-cost / slippage params
pub struct RegimeStress;             // replay strategy against labeled stress windows
pub struct VolatilityShock;
pub struct CorrelationShock;
```

Output is a **distribution**, not a point estimate:

```rust
pub struct SimulationResult {
    pub sharpe_distribution: Distribution,
    pub drawdown_distribution: Distribution,
    pub cagr_distribution: Distribution,
    pub tail_risk_distribution: Distribution,
    pub prob_of_ruin: f64,
}
```

The CLI report (`quantctl simulate`) surfaces percentile bands (p5/p50/p95) —
a single "beautiful" equity curve from `quantctl backtest` is explicitly
paired with this distribution before being trusted.

## Regime Analysis

```rust
pub trait RegimeDetector {
    fn label(&self, market_state: &MarketState) -> Regime;   // Bull, Bear, HighVol, LowVol, Trending, MeanReverting, Crisis
}
```

No detector implementation is hard-coded as canonical (an HMM-based detector
is one possible implementation, not baked into the trait) — `BacktestReport`
can be sliced by regime label for any detector plugged in, showing whether the
strategy is regime-dependent rather than robust.

## Statistical Rigor Guardrails

- `quantctl backtest run` refuses to run against the **test** split more than
  once per model artifact version without an explicit `--allow-reuse` flag and
  a warning — discourages implicit test-set hyperparameter tuning.
- `quantctl report` includes a probability-of-backtest-overfitting estimate
  (deflated Sharpe ratio, given the number of prior trials recorded in the
  experiment log from doc `03-MODEL-TRAINING.md`) alongside headline metrics.
- Parameter-sensitivity sweeps (`quantctl simulate --sweep`) are a first-class
  command, not a manual notebook exercise — the platform expects it to be run
  before any strategy is considered validated.
