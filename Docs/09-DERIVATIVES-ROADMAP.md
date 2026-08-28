# 09 — Derivatives Roadmap

## Phasing

```
Phase 1 (now)   : Equities only
Phase 2         : Futures
Phase 3         : Options
Phase 4         : Portfolio-level derivatives (multi-instrument, cross-greeks)
```

No phase requires rewriting `signals/`, `portfolio/`, `risk/`, `execution/`, or
`backtest/` — those already operate on `dyn Instrument` and `Signal`, not on
`Equity` directly (doc 05). What each phase actually adds:

## Phase 2 — Futures

- New `Instrument` impl: `Future { underlying, expiry, multiplier, tick_size,
  contract_month }`.
- New `MarketDataProvider` fields: continuous contract handling, roll logic —
  added as a `RollPolicy` trait so calendar-spread vs. volume-based rolls are
  swappable.
- Execution model gains margin/leverage awareness (`ExecutionModel` already
  takes `market_state`; futures margin becomes another field there).
- No change to feature engine, model, or backtest loop structure — a futures
  price series feeds the same `Bar`/`Feature` pipeline as equities.

## Phase 3 — Options

- New `Instrument` impl: `Option_ { underlying, strike, expiry, option_type,
  multiplier }` (struct already sketched in doc 05).
- New data types:
  ```rust
  pub struct OptionChain { pub underlying: InstrumentId, pub as_of: Timestamp,
                          pub contracts: Vec<OptionQuote> }
  pub struct OptionQuote { pub contract: Option_, pub bid: f64, pub ask: f64,
                          pub last: f64, pub iv: f64, pub open_interest: u64,
                          pub volume: u64, pub greeks: Greeks }
  pub struct Greeks { pub delta: f64, pub gamma: f64, pub theta: f64,
                      pub vega: f64, pub rho: f64 }
  ```
- New `OptionPricingModel` trait (Black-Scholes first, Binomial/Monte
  Carlo/local-vol as later implementations) — used both to fill in missing
  Greeks from vendor data and to price hypothetical strategies in simulation.
- `RiskReport::factor_exposure` (already present, doc 05) gains `delta`,
  `gamma`, `vega`, `theta` keys — no schema migration, just new keys.
- The LSTM (or any `PredictionProvider`) still predicts underlying returns;
  a new `SignalTransform` (doc 05) maps a directional/volatility signal into
  an options strategy selection (e.g., predicted high vol → long straddle),
  keeping the model itself instrument-agnostic.

## Phase 4 — Portfolio Derivatives

- Cross-instrument risk aggregation: portfolio-level Greeks, correlation
  across equities/futures/options positions — an extension of the
  `RiskEngine`'s existing `correlation_matrix` and `factor_exposure` fields to
  span heterogeneous instrument types.
- Multi-leg order generation (spreads, collars) — an extension of the
  `Order`/`Fill` types (doc 05) to represent a basket atomically, so execution
  simulation can model leg-fill risk (one leg fills, the other doesn't).

## What Is Deliberately Not Built Now

- No option-chain ingestion adapter, no live Greeks computation, no
  vol-surface modeling in Phase 1. Building these before there's a working
  equity signal-to-backtest pipeline would be premature complexity — the
  point of this roadmap is that the *interfaces* already accommodate them,
  not that the code exists yet.
