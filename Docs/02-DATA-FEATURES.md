# 02 — Data Engine & Feature Engine (Rust)

## `MarketDataProvider` Trait

```rust
pub trait MarketDataProvider {
    fn fetch_ohlcv(&self, symbol: &str, start: Date, end: Date) -> Result<Vec<Bar>>;
    fn corporate_actions(&self, symbol: &str, start: Date, end: Date) -> Result<Vec<CorporateAction>>;
    fn trading_calendar(&self, exchange: &str) -> Result<TradingCalendar>;
}

pub struct Bar {
    pub timestamp: Timestamp,   // exchange-local, normalized to UTC at ingestion
    pub open: f64, pub high: f64, pub low: f64, pub close: f64,
    pub volume: u64,
    pub adjusted: bool,
}
```

- `YfinanceAdapter` implements this trait and lives under `data/adapters/` as an
  explicitly labeled **free/experimental** source — never referenced by name outside
  the adapter module. Everything else in the system depends on the trait.
- Additional adapters (paid vendor feeds, broker APIs) implement the same trait —
  zero downstream changes required.

## Point-In-Time (PIT) Correctness

This is enforced structurally, not by convention:

- Every `Bar` and every computed `Feature` carries an **availability timestamp**
  distinct from its **observation timestamp**. A feature is only usable for a
  prediction at time `T` if `availability_timestamp <= T`.
- The dataset builder rejects (with an explicit error, not a silent drop) any
  row where a feature's availability timestamp is later than the target's
  observation window.
- Corporate actions (splits/dividends) are applied as of their **effective date**,
  never retroactively re-adjusting historical bars in a way that would leak
  future-known adjustment factors into past feature values.
- Universe membership is point-in-time: a symbol used in training at date `T`
  must have actually been tradable/listed at `T` (no survivorship bias from using
  today's S&P 500 constituents to backtest 2015).

## Leakage Checklist (Automated, Not Documentation-Only)

Implemented as a validation pass (`data::validate::check_leakage`) that runs on
every dataset build and **fails the build** on violation:

- feature availability timestamp vs. target timestamp
- duplicate timestamps per (asset, field)
- train/validation/test index overlap
- scaler-fit boundary (scaler must only ever be `fit` on a train split, never
  refit during val/test/inference — enforced by the scaler type only exposing
  `fit_transform` on `TrainSplit` and `transform` on `ValSplit`/`TestSplit`, a
  compile-time distinction, not a runtime check)
- universe membership at the observation date

## Feature Engine

```rust
pub trait Feature {
    fn name(&self) -> &str;
    fn version(&self) -> u32;
    fn lookback(&self) -> usize;
    fn compute(&self, window: &BarWindow) -> f64;
    fn availability_offset(&self) -> Duration; // usually zero (same-bar close) — explicit if not
}
```

- `FeatureGraph` resolves dependency order (e.g., MACD depends on two EMAs) and
  computes each feature once per timestamp, memoized — no recomputation across
  overlapping windows.
- `FeatureSet` is a named, versioned bundle (e.g., `"baseline_v1"` = the 9
  indicators from the prior design: log return, rolling vol, RSI, MACD diff,
  Bollinger width, ATR, ADX, OBV z-score, volume ratio).
- `FeatureStore` persists computed features to Parquet, partitioned by
  `(feature_set_version, symbol, date)` — recomputation is skip-if-cached.
- Changing a feature's definition **must** bump its `version()` — the store
  keys on version, so old and new definitions coexist rather than silently
  overwriting history (a subtle reproducibility bug in the prior Python
  design, where re-running `add_indicators` with modified logic would silently
  overwrite prior results).

## Normalization

- `Scaler::fit(&TrainSplit) -> FittedScaler` — the only way to produce a fitted
  scaler; there is no `fit` method reachable from validation, test, or inference
  code paths.
- `FittedScaler` is serialized **inside the model artifact** (see
  `03-MODEL-TRAINING.md`), not as a separate file that could drift out of sync
  with the model weights.
- Inference always deserializes the scaler bundled with the specific model
  version being served — it is structurally impossible for production
  inference to fit its own normalization.

## Memory Layout

- Feature store uses **columnar Parquet / Arrow** (SoA — struct-of-arrays), not
  row-major structs, so a backtest that only needs `close` and `predicted_return`
  doesn't pay for loading all 9 feature columns into cache.
- Sequence construction for the LSTM (`(N, lookback, n_features)`) is the one
  place we deliberately go row-major/contiguous, because the LSTM consumes
  contiguous per-timestep feature vectors — this conversion happens once, at
  dataset-build time, not per-training-epoch.
- Rolling-window computations (RSI, ATR, Bollinger, etc.) use ring buffers
  sized to `lookback`, avoiding reallocation per timestep.

## Data Contract (Training Dataset)

```
timestamp     : i64 (unix ns, UTC)
asset_id      : u32
feature_vector: [f32; N]   // N = feature_set version's dimensionality
target        : f32        // e.g., next-1-day log return
target_horizon: u16        // days, supports multi-horizon targets later
sample_weight : f32        // default 1.0; reserved for future regime/vol weighting
feature_set_version: u32
```

Stored as Arrow IPC. This is the exact file handed to Python for training — the
only artifact Python reads from the Rust side of the system.

## Multi-Asset / Cross-Sectional Readiness

- `asset_id` is a first-class column from day one, even though phase-1 usage is
  single- or few-symbol.
- Cross-sectional features (market-relative returns, sector rank, beta) are
  defined as a `Feature` variant that takes a `Universe` snapshot at time `T`
  rather than a single-asset `BarWindow`, so ranking/long-short strategies don't
  require a schema change later — only new `Feature` implementations.
