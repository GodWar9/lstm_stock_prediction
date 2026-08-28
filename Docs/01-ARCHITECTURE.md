# 01 — Final Architecture

## Language Boundary (Non-Negotiable)

```
PYTHON                              RUST
┌──────────────────────┐            ┌──────────────────────────┐
│ ML experimentation    │            │ Market data engine        │
│ PyTorch model def      │            │ Feature engine             │
│ Training loop          │  Model    │ Normalization (apply-only) │
│ Walk-forward driver     │─Artifact─▶│ Sequence builder           │
│ Hyperparameter search  │            │ ONNX Runtime inference     │
│ Model validation        │            │ Signal engine               │
│ Model export (ONNX)     │            │ Portfolio engine            │
└──────────────────────┘            │ Risk engine                 │
                                     │ Execution simulator          │
                                     │ Backtest engine              │
                                     │ Monte Carlo / simulation      │
                                     │ CLI (quantctl) / API layer   │
                                     │ Config loader & validation    │
                                     └──────────────────────────┘
```

**Rule:** Python never touches market data, features, backtesting, portfolio, risk,
or execution. Its only inputs/outputs are: a training dataset (Arrow/Parquet, built
by Rust) in, a versioned model artifact (ONNX + metadata) out. Rust never trains
a model or defines model architecture.

## System Data Flow

```
Market Data Provider (pluggable: yfinance adapter today, others later)
        │
        ▼
Rust: Data Validation  ──▶  rejects bad ticks, dup timestamps, PIT violations
        │
        ▼
Rust: Feature Engine  ──▶  versioned features, Parquet feature store
        │
        ▼
Rust: Dataset Builder ──▶  Arrow IPC training dataset (timestamp, asset_id, features, target)
        │
        ▼ (handoff — Arrow IPC file, not a live process boundary)
Python: PyTorch Training  ──▶  walk-forward loop, experiment tracking
        │
        ▼
Python: Export  ──▶  versioned Model Artifact (ONNX + metadata.json)
        │
        ▼ (handoff — artifact directory, read-only from here on)
Rust: Inference Runtime (ONNX Runtime CPU, session loaded once)
        │
        ▼
Rust: Signal Engine  ──▶  Signal { direction, expected_return, confidence, horizon, instrument }
        │
        ▼
Rust: Portfolio Engine  ──▶  target positions under constraints
        │
        ▼
Rust: Execution Simulator  ──▶  fills, slippage, market impact
        │
        ▼
Rust: Backtest / Risk Engine  ──▶  PnL, drawdown, VaR, exposure
        │
        ▼
Rust: Simulation Engine  ──▶  Monte Carlo, stress, regime analysis
        │
        ▼
Reports / Analytics (CLI output, artifacts on disk)
```

## Why This Boundary (Not a Rust HTTP Wrapper Around Python)

- The Python↔Rust interface only crosses **twice**: dataset-in, artifact-out. No
  per-prediction serialization tax, no process-per-request overhead.
- Every performance-sensitive path (feature computation, backtest loop, execution
  simulation, Monte Carlo) is Rust from day one — not retrofitted later.
- The model becomes genuinely swappable: `PredictionProvider` is a Rust trait;
  LSTM/XGBoost/linear/ensemble models all produce the same artifact shape and are
  indistinguishable to everything downstream.
- The backtester never imports `model/lstm.py` — it only knows `SignalStream`.

## Repository Structure

```
lstm-quant-platform/
│
├── python/
│   ├── ml/
│   │   ├── models/          # LSTMPredictor, future model classes
│   │   ├── training/        # trainer, walk-forward driver, loss fns
│   │   ├── validation/      # metric computation during training only
│   │   ├── experiments/     # experiment tracking (config+seed+metrics records)
│   │   └── export/          # ONNX export + artifact packaging
│   └── research/            # notebooks, exploratory scripts — never imported by production
│
├── rust/
│   ├── data/                # MarketDataProvider trait + adapters (yfinance, future: paid feeds)
│   ├── calendar/             # trading calendars, sessions, holidays
│   ├── instruments/          # Instrument, Equity, Future, Option abstractions
│   ├── features/             # Feature, FeatureGraph, FeatureStore
│   ├── inference/            # ONNX Runtime session wrapper, PredictionProvider trait
│   ├── signals/               # Signal struct, signal transformations
│   ├── portfolio/             # position sizing, constraints, exposure rules
│   ├── risk/                  # VaR, CVaR, drawdown, factor exposure
│   ├── execution/              # ExecutionModel trait: spread/slippage/impact
│   ├── backtest/               # SignalStream consumer, PnL engine
│   ├── simulation/             # Monte Carlo, bootstrap, stress scenarios
│   ├── config/                  # single config loader (YAML + env + CLI overrides), validated at startup
│   └── cli/                     # quantctl — the one entrypoint
│
├── models/                     # versioned model artifacts (ONNX + metadata.json)
├── datasets/                   # versioned Arrow/Parquet datasets
├── configs/                    # config.yaml + environment overrides
├── tests/                      # unit, property, integration, golden, parity
├── benchmarks/                 # criterion (Rust) + pytest-benchmark (Python)
├── docs/                       # this doc set
└── README.md
```

## Document Map

| Doc | Covers |
|---|---|
| `00-AUDIT.md` | This audit |
| `01-ARCHITECTURE.md` | This file |
| `02-DATA-FEATURES.md` | Data engine, PIT correctness, feature engine (Rust) |
| `03-MODEL-TRAINING.md` | PyTorch model, walk-forward CV, artifact versioning (Python) |
| `04-INFERENCE.md` | Rust ONNX inference core — **not a stretch goal** |
| `05-PORTFOLIO-RISK-EXECUTION.md` | Signal → Portfolio → Risk → Execution layers |
| `06-BACKTEST-SIMULATION.md` | Backtest engine, Monte Carlo, regime analysis |
| `07-PERFORMANCE-TESTING.md` | CPU optimization plan + testing strategy |
| `08-DEPLOYMENT-CONFIG.md` | `quantctl` CLI, config system, observability, failure handling |
| `09-DERIVATIVES-ROADMAP.md` | Options/futures extension path |
| `10-MIGRATION-PLAN.md` | Staged migration from the old Python-only version |
