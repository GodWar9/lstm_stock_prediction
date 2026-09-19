# Quantitative Research & Deterministic Backtesting Platform

An institutional-grade, high-performance quantitative trading research, machine learning training, and deterministic backtesting platform. Engineered with a strict two-language architecture: **Rust** for data validation, zero-allocation feature engineering, low-latency ONNX Runtime CPU inference, portfolio risk management, and execution simulation; and **Python (PyTorch)** exclusively for deep learning experimentation, loss function design, and walk-forward cross-validation.

---

## Key Highlights

- **Strict Language Boundary**: Rust owns data ingestion, feature generation, runtime inference, signals, portfolio, risk, execution simulation, and backtesting. Python only consumes Arrow datasets and outputs versioned ONNX model packages.
- **Point-In-Time (PIT) Correctness**: Indicators and sliding windows operate with circular ring buffers without lookahead bias. All scalers are fit strictly on training splits.
- **Zero-Copy Arrow IPC Contract**: Rust builds typed Apache Arrow IPC training datasets directly consumed by PyTorch without intermediate serialization bottlenecks.
- **PyTorch LSTM with Directional Asymmetric Loss**: Multi-layer LSTM model with Greff/Gers forget gate initialization, Huber base loss, and sign-disagreement penalties.
- **Versioned ONNX & Dynamic INT8 Quantization**: Sub-millisecond CPU inference via Microsoft ONNX Runtime (`ort` crate) with verified numerical parity ($\epsilon < 10^{-5}$) against PyTorch.
- **Realistic Execution Simulation**: Models quadratic market impact, bid-ask spread costs, fixed broker commissions, and volume participation caps.
- **Statistical Robustness**: Stationary bootstrap Monte Carlo resampling and market regime classification (Bull, Bear, Sideways, Crisis).
- **Unified CLI (`quantctl`)**: Single command-line interface controlling ingestion, validation, feature computation, training, inference, backtesting, and reporting.

---

## Architectural Overview

```text
┌─────────────────────────────────────────────────────────────────────────────────┐
│                               LANGUAGE BOUNDARY                                 │
├───────────────────────────────────────┬─────────────────────────────────────────┤
│         PYTHON (Model Research)       │       RUST (Production Engine)          │
├───────────────────────────────────────┼─────────────────────────────────────────┤
│ • PyTorch LSTM architecture           │ • Market data ingestion & PIT validation│
│ • Directional asymmetric loss         │ • Zero-allocation streaming indicators  │
│ • Purged/embargoed walk-forward CV    │ • Parquet feature store & Arrow export  │
│ • Arrow dataset consumption           │ • ONNX Runtime CPU inference engine     │
│ • Model checkpointing & early stopping│ • Signal calibration & ranking pipeline │
│ • ONNX export & INT8 quantization     │ • Portfolio constructor & risk limits   │
│ • Out-of-sample ML evaluation         │ • Execution simulation & slippage models│
│                                       │ • Event-driven backtesting engine       │
│                                       │ • Monte Carlo resampling & regime tests │
│                                       │ • `quantctl` unified CLI entrypoint     │
└───────────────────────────────────────┴─────────────────────────────────────────┘
```

### Data & Model Flow

```text
Market Data Provider (yfinance / mock)
        │
        ▼
Rust: Data Validation ──▶ Rejects non-monotonic ticks, duplicate timestamps, invalid OHLC
        │
        ▼
Rust: Feature Engine ──▶ Computes SMA, EMA, RSI, Bollinger, MACD, ATR, Volatility
        │
        ▼
Rust: Dataset Builder ──▶ Exports typed Arrow IPC dataset (features + forward targets)
        │
        ▼ (Boundary 1: Arrow IPC file)
Python: PyTorch Training ──▶ Purged walk-forward CV, directional loss, early stopping
        │
        ▼ (Boundary 2: Versioned ONNX artifact + metadata.json + scaler.json)
Rust: ONNX Runtime Inference ──▶ Multi-threaded SIMD CPU inference (`ort`)
        │
        ▼
Rust: Signal Engine ──▶ Direction, expected return, confidence, deadband filter
        │
        ▼
Rust: Portfolio Engine ──▶ Volatility targeting, gross/net leverage constraints
        │
        ▼
Rust: Execution Simulator ──▶ Half-spread bps, fixed commissions, quadratic slippage
        │
        ▼
Rust: Backtest & Risk Engine ──▶ Equity curve, Sharpe/Sortino, VaR/CVaR, max drawdown
        │
        ▼
Rust: Simulation Engine ──▶ Stationary bootstrap Monte Carlo & regime classification
```

---

## Repository Structure

```text
├── configs/
│   └── default.yaml                     # Unified system configuration
├── Docs/                                # Architectural & design documentation
│   ├── 00-AUDIT.md                      # Codebase audit & migration rationale
│   ├── 01-ARCHITECTURE.md               # Target architecture & language boundaries
│   ├── 02-DATA-FEATURES.md              # Data validation & feature engineering
│   ├── 03-MODEL-TRAINING.md             # Model training, loss functions & CV
│   ├── 04-INFERENCE.md                  # Rust ONNX Runtime inference core
│   ├── 05-PORTFOLIO-RISK-EXECUTION.md   # Signal -> Portfolio -> Risk -> Execution
│   ├── 06-BACKTEST-SIMULATION.md        # Event backtesting & Monte Carlo specs
│   ├── 07-PERFORMANCE-TESTING.md        # CPU optimization & testing strategy
│   ├── 08-DEPLOYMENT-CONFIG.md          # CLI deployment & telemetry
│   ├── 09-DERIVATIVES-ROADMAP.md        # Options & futures roadmap
│   ├── 10-MIGRATION-PLAN.md             # Staged migration blueprint
│   ├── 11-IMPLEMENTATION-PLAN.md        # Active milestone backlog
│   └── 12-IMPLEMENTATION-CHANGELOG.md   # Detailed engineering changelog
├── datasets/                            # Versioned data artifacts (git-ignored)
│   ├── market/                          # Validated OHLCV JSON + manifests
│   ├── features/                        # Parquet feature tables
│   └── training/                        # Causal Arrow IPC training sets
├── models/
│   └── lstm_v1/                         # Versioned model artifacts (ONNX + metadata)
├── python/
│   ├── ml/                              # PyTorch ML subsystem
│   │   ├── arrow_dataset.py             # Rust Arrow IPC reader & validation
│   │   ├── dataset.py                   # Purged walk-forward CV splitter & scaling
│   │   ├── evaluate.py                  # IC, Rank IC, Sharpe & accuracy metrics
│   │   ├── export_onnx.py               # ONNX exporter with metadata embedding
│   │   ├── loss.py                      # DirectionalAsymmetricLoss & SharpeAwareLoss
│   │   ├── quantize.py                  # Dynamic INT8 post-training quantization
│   │   ├── trainer.py                   # ModelTrainer with early stopping
│   │   ├── train_orchestrator.py        # End-to-end training entrypoint
│   │   └── models/lstm.py               # PyTorch LSTMForecaster architecture
│   ├── research/legacy_v1/              # Frozen historical prototype (reference only)
│   └── tests/                           # Python unit, parity, and leakage tests
├── reports/                             # Generated backtest & simulation reports
└── rust/                                # Rust Core Production Workspace
    ├── backtest/                        # Event-driven backtesting engine & PnL reporter
    ├── calendar/                        # NYSE/NASDAQ trading calendar & holidays
    ├── cli/                             # quantctl CLI binary & command handlers
    ├── config/                          # YAML/Env config loader & startup validator
    ├── data/                            # Market data providers, validation & storage
    ├── execution/                       # Execution simulator (spread, slippage, impact)
    ├── features/                        # 9 technical indicators, ring buffers & Arrow IPC
    ├── inference/                       # ONNX Runtime CPU session & FittedScaler
    ├── instruments/                     # Equity, Option & Future models with Greeks
    ├── portfolio/                       # Volatility-targeted position sizing
    ├── risk/                            # VaR (95/99), CVaR, drawdown & leverage limits
    ├── signals/                         # Signal calibration, deadband & rank transforms
    └── simulation/                      # Monte Carlo bootstrap & regime classifier
```

---

## Prerequisites & Installation

### 1. System Requirements
- **Rust**: 1.70+ stable (`rustup default stable`)
- **Python**: 3.10, 3.11, or 3.14
- **C++ Build Tools**: MSVC v143+ (Windows) or `build-essential` (Linux)

### 2. Python Setup
Create and activate a virtual environment, then install requirements:
```bash
python -m venv .venv
# On Windows PowerShell:
.venv\Scripts\Activate.ps1
# On Linux / macOS:
source .venv/bin/activate

pip install -r requirements.txt
```

### 3. Rust Workspace Verification
Verify that the workspace compiles and all tests pass:
```bash
cd rust
cargo test --workspace
```

---

## Operational Runbook: Quickstart

All operations are executed via the `quantctl` CLI binary.

### 1. Ingest & Validate Market Data
```bash
# Fetch historical data
cargo run --bin quantctl -- data fetch --symbol AAPL --start 2015-01-01 --end 2024-01-01 --version ds_2024_v1

# Validate integrity, monotonicity, and candle sanity
cargo run --bin quantctl -- data validate --dataset ds_2024_v1 --symbol AAPL
```

### 2. Build Features & Arrow IPC Training Dataset
```bash
# Computes technical indicators, forward targets, and exports versioned Arrow IPC dataset
cargo run --bin quantctl -- features build --symbol AAPL --dataset ds_2024_v1 --feature-set baseline_v1
```

### 3. Train Model in PyTorch & Export ONNX
```bash
# Trains LSTMForecaster with purged CV, early stopping, and exports ONNX + INT8 models
cargo run --bin quantctl -- train --dataset ds_2024_v1 --symbol AAPL --epochs 50 --batch-size 64
```

### 4. Run Rust ONNX Inference
```bash
# Runs low-latency native inference in Rust using ONNX Runtime
cargo run --bin quantctl -- predict --symbol AAPL --model lstm_v1
```

### 5. Run Deterministic Event-Driven Backtest
```bash
# Backtests over historical bars with slippage, transaction costs, and volatility targeting
cargo run --bin quantctl -- backtest --symbol AAPL --start 2023-01-01 --end 2024-01-01
```

### 6. Run Monte Carlo Simulation & Regime Analysis
```bash
# Resamples historical returns across 1,000 bootstrap runs and detects market regimes
cargo run --bin quantctl -- simulate --symbol AAPL --runs 1000
```

### 7. Generate Performance Report
```bash
# Emits summary analytics, Sharpe ratio, Sortino, drawdowns, and VaR/CVaR
cargo run --bin quantctl -- report --symbol AAPL
```

---

## Testing & Quality Gates

The codebase enforces rigorous multi-layer testing:

- **Indicator Golden Tests**: Closed-form mathematical verification for SMA, EMA, Bollinger Bands, and Wilder's RSI smoothing.
- **Property-Based Tests**: Uses `proptest` to fuzz test timestamp monotonicity, duplicate rejection, and candle bounds.
- **Temporal Leakage Tests**: Validates zero feature leakage across train/validation splits, purge gaps, and embargo buffers.
- **Numerical Parity Tests**: Validates that PyTorch and ONNX Runtime CPU outputs match across batch sizes and sequence lengths ($\epsilon < 10^{-5}$).
- **Integration Tests**: End-to-end execution of `quantctl` CLI commands and pipeline components.

To run all automated suites:
```bash
# Rust Workspace Tests (70+ tests)
cargo test --workspace

# Python Parity & Leakage Tests (11 tests)
pytest python/tests
```

---

## License

MIT OR Apache-2.0
