# 05 — Signal, Portfolio, Risk & Execution (Rust)

## Instrument Abstraction (Derivatives-Ready From Day One)

```rust
pub trait Instrument {
    fn id(&self) -> InstrumentId;
    fn asset_class(&self) -> AssetClass;   // Equity, Etf, Future, Option, Index, Fx, Crypto
    fn currency(&self) -> Currency;
    fn exchange(&self) -> &str;
    fn multiplier(&self) -> f64;
    fn tick_size(&self) -> f64;
}

pub struct Equity { pub id: InstrumentId, pub symbol: String, /* ... */ }

pub struct Option_ {
    pub underlying: InstrumentId,
    pub strike: f64,
    pub expiry: Date,
    pub option_type: OptionType,   // Call, Put
    pub multiplier: f64,
    // greeks, iv, open_interest populated when option-chain data is wired in — Phase 3
}
```

Phase 1 ships only `Equity`. The trait boundary is what prevents Phase 3
(options) from requiring a rewrite of portfolio/risk/execution — those layers
already operate on `dyn Instrument`, never on `Equity` directly.

## Signal Engine

Already defined in `04-INFERENCE.md` (`Signal` struct). This layer's job is
transformation, not generation:

```rust
pub trait SignalTransform {
    fn apply(&self, raw: &Signal) -> Signal;
}
// e.g. ThresholdFilter, ConfidenceWeighting, CrossSectionalRank
```

## Portfolio Engine

Decoupled from the model entirely — consumes `Signal`, not `Prediction`.

```rust
pub struct PortfolioConstraints {
    pub max_gross_exposure: f64,
    pub max_net_exposure: f64,
    pub max_position_pct: f64,      // per-instrument cap
    pub max_sector_pct: f64,
    pub max_turnover_pct: f64,
    pub volatility_target: Option<f64>,
    pub long_short_mode: LongShortMode,  // LongOnly, LongShort, DollarNeutral, BetaNeutral
}

pub trait PortfolioConstructor {
    fn target_positions(&self, signals: &[Signal], current: &Portfolio,
                        constraints: &PortfolioConstraints) -> TargetPositions;
}
```

- Position sizing is never "1 unit per signal" (the old design's flaw) — it's
  a function of confidence, volatility targeting, and constraint satisfaction.
- `max_drawdown` breach triggers a configurable de-risking rule (reduce gross
  exposure, not necessarily flatten) evaluated by the risk engine below.

## Risk Engine

Standalone module, consumed by both the live portfolio constructor and the
backtest loop identically — risk numbers must mean the same thing in both.

```rust
pub struct RiskReport {
    pub var_95: f64,
    pub cvar_95: f64,
    pub volatility: f64,
    pub max_drawdown: f64,
    pub beta: f64,
    pub correlation_matrix: Array2<f64>,
    pub gross_exposure: f64,
    pub net_exposure: f64,
    pub turnover: f64,
    pub factor_exposure: HashMap<String, f64>,   // populated as factor models are added
}
```

`factor_exposure` and a `stress_scenarios: Vec<StressResult>` field are present
from Phase 1 even though only a couple of entries are populated initially —
this is the seam where options Greeks (`delta`, `gamma`, `vega`, `theta`) get
added in Phase 3 as additional keys, not a schema migration.

## Execution Model (Extensible Cost Abstraction)

```rust
pub trait ExecutionModel {
    fn simulate_fill(&self, order: &Order, market_state: &MarketState) -> Fill;
}

pub struct CompositeExecutionModel {
    pub fixed_cost: FixedCost,
    pub spread: SpreadModel,
    pub slippage: SlippageModel,
    pub participation: VolumeParticipationModel,
    pub impact: Option<Box<dyn MarketImpactModel>>,   // optional, Phase 2+
}
```

- Default Phase-1 model: fixed commission + half-spread cost + linear slippage
  proportional to order size vs. average volume. Deliberately not claiming
  market-impact realism until it's benchmarked against real fill data.
- `Order` supports `Market` and `Limit` types and a `participation_cap` so a
  backtest can't silently assume infinite same-bar liquidity — this replaces
  the old "everything fills at close" assumption.

## Data Flow Through This Layer

```
Prediction (from 04-INFERENCE.md)
      ↓
Signal
      ↓ SignalTransform (threshold / confidence weighting)
Signal (refined)
      ↓ PortfolioConstructor + PortfolioConstraints
TargetPositions
      ↓ Order generation (diff vs. current positions)
Order[]
      ↓ ExecutionModel::simulate_fill
Fill[]
      ↓
Position update → PnL → RiskReport
```

This entire chain is what `06-BACKTEST-SIMULATION.md` drives historically and
what a future live-trading adapter would drive in real time — same code path,
different `MarketDataProvider` and `ExecutionModel` implementations.
