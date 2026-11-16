> Superseded by [the dated handover](HANDOVER-2026-11-16.md) and [current claims](CLAIMS.md). Historical dates and test counts below are not current acceptance evidence.

# Engineering Handover & Platform Architecture Specification

> **Platform**: Quantitative Research & Deterministic Backtesting Engine (Rust + PyTorch/ONNX)  
> **Repository**: `lstm_stock_prediction`  
> **Workspace**: `c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction`  
> **Date**: September 2026  
> **Git Baseline**: Commit `3dcca43` (67 verified commits, full green test suite)

---

## 1. Executive Summary & Architecture Philosophy

This platform is an quantitative research, machine learning training, and deterministic backtesting engine. It was transformed from an exploratory Python script into a dual-language, research system adhering to strict institutional separation of concerns:

```text
┌─────────────────────────────────────────────────────────────────────────────────┐
│                               LANGUAGE BOUNDARY                                 │
├───────────────────────────────────────┬─────────────────────────────────────────┤
│         PYTHON (Model Research)       │       RUST (Production Engine)          │
├───────────────────────────────────────┼─────────────────────────────────────────┤
│ • PyTorch LSTM architecture           │ • Market data ingestion & PIT validation│
│ • Custom asymmetric loss functions    │ • Zero-allocation streaming indicators  │
│ • Purged/embargoed walk-forward CV    │ • Parquet feature store & Arrow export  │
│ • PyArrow dataset consumption         │ • tract CPU inference engine     │
│ • Model checkpointing & early stopping│ • Signal calibration & ranking pipeline │
│ • ONNX export & INT8 quantization     │ • Portfolio constructor & risk limits   │
│ • Out-of-sample ML evaluation         │ • Execution simulation & slippage models│
│                                       │ • Event-driven backtesting engine       │
│                                       │ • Monte Carlo resampling & regime tests │
│                                       │ • `quantctl` unified CLI entrypoint     │
└───────────────────────────────────────┴─────────────────────────────────────────┘
```

### Core Invariants & Architectural Rules
1. **Zero Market Data in Python**: Python never connects directly to market data feeds or raw historical databases. Python only consumes validated, versioned **Apache Arrow IPC** datasets produced by the Rust feature engine.
2. **Zero Python in Execution**: Python is completely absent from the inference runtime, signal processing, portfolio sizing, risk checking, and execution backtest loop.
3. **Artifact-Driven Handoff**: The handoff between Python and Rust crosses only twice:
   - **Data Handoff**: Rust produces `datasets/training/<dataset_version>/<symbol>.arrow` + `.manifest.json`.
   - **Model Handoff**: Python produces `models/<model_id>/model.onnx`, `metadata.json`, `scaler.json`, and `model_int8.onnx`.
4. **Point-In-Time (PIT) Correctness**: Indicators and features are calculated exclusively using strictly causal lookbacks with zero lookahead bias. Scalers are fit strictly on training splits and serialized for apply-only inference in Rust.

---

## 2. Phase-Wise Implementation Plan: Current Status & Progress

The platform follows a 10-phase roadmap (aligning with Stages 0–7 of the migration plan). The table below details what has been built, the current state, and the remaining milestones:

| Phase | Title | Status | Primary Crates / Modules | Verified Commits / Milestones |
|---|---|---|---|---|
| **Phase 1** | Foundation, Config, CI & CLI Skeleton | **Complete** | [`quant_config`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/config), [`quantctl`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/cli) | Workspace setup, YAML/Env loader, CLI commands, telemetry |
| **Phase 2** | Versioned Market Data & Strict Ingestion | **Complete** | [`quant_data`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/data) | `DatasetManifest`, OHLCV validation, duplicate rejection |
| **Phase 3** | Point-In-Time & Leakage Validation | **Complete** | [`quant_data`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/data), [`python/ml/dataset.py`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/python/ml/dataset.py) | Lookahead guards, purge/embargo splits, scaler isolation |
| **Phase 4** | Rust Feature Store & Arrow IPC Builder | **Complete** | [`quant_features`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/features) | 9 indicators, golden tests, Parquet & Arrow IPC manifests |
| **Phase 5** | PyTorch Training, Custom Loss & Orchestration | **Complete** | [`python/ml`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/python/ml), [`quantctl train`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/cli/src/handlers/train.rs) | LSTM with forget gates, directional loss, Arrow dataset loader |
| **Phase 6** | ONNX Export, Parity & Rust Inference | **Complete** | [`quant_inference`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/inference), [`python/ml/export_onnx.py`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/python/ml/export_onnx.py) | PyTorch vs ONNX parity tests, `tract-onnx` CPU session, INT8 quant |
| **Phase 7** | Signal Generation & Transforms | **Complete** | [`quant_signals`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/signals) | Signal calibration, deadband filters, rank transforms |
| **Phase 8** | Portfolio, Risk & Execution Backtester | **Complete** | [`quant_portfolio`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/portfolio), [`quant_risk`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/risk), [`quant_execution`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/execution), [`quant_backtest`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/backtest) | Event backtest loop, slippage/impact, VaR/CVaR, drawdowns |
| **Phase 9** | Monte Carlo Simulation & Regime Analysis | **Complete** | [`quant_simulation`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/simulation) | Stationary bootstrap resampling, bull/bear/crisis regimes |
| **Phase 10**| Production Polish, CI Gates & Derivatives | **In Progress** | [`quant_instruments`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/instruments), [`Docs`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/Docs) | Equity + Options/Futures abstractions, CI pipelines |

---

## 3. Deep Dive into Built Subsystems

### 3.1 Data Ingestion & Storage ([`quant_data`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/data))
- **Ingestion Trait**: [`MarketDataProvider`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/data/src/provider.rs) implemented for [`YfinanceAdapter`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/data/src/adapters/yfinance.rs) and [`MockAdapter`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/data/src/adapters/mock.rs).
- **Validation Engine**: [`DataValidator`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/data/src/validate.rs) enforces:
  - Strictly monotonic timestamps (no timestamp reversals or duplicates).
  - Finite positive prices and non-negative volume.
  - Consistent candle relationships: $\text{high} \ge \max(\text{open}, \text{close})$ and $\text{low} \le \min(\text{open}, \text{close})$.
- **Deterministic Persistence**: [`DatasetStorage`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/data/src/storage.rs) stores OHLCV bars in `datasets/market/<version>/<symbol>.json` accompanied by a [`DatasetManifest`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/data/src/storage.rs) tracking row count, date ranges, and SHA-256 metadata.
- **Fail-Fast Policy**: Removed silent fallbacks to synthetic data; real ingestion errors halt execution immediately.

### 3.2 Feature Engine & Arrow IPC Dataset Builder ([`quant_features`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/features))
- **Zero-Allocation Windowing**: [`Window<T>`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/features/src/window.rs) implements circular ring buffers for streaming $O(1)$ updates.
- **Indicators Implemented**:
  - [`SimpleMovingAverage`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/features/src/moving_avg.rs) & [`ExponentialMovingAverage`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/features/src/moving_avg.rs)
  - [`BollingerBands`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/features/src/bollinger.rs) (mean, upper, lower, bandwidth, %B)
  - [`RelativeStrengthIndex`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/features/src/rsi.rs) with Wilder's exponential smoothing
  - [`MovingAverageConvergenceDivergence`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/features/src/macd.rs) (MACD line, signal line, histogram)
  - [`AverageTrueRange`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/features/src/atr.rs) & True Range
  - [`RollingVolatility`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/features/src/volatility.rs) & [`LogReturn`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/features/src/volatility.rs)
- **Target Generation**: [`TargetGenerator`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/features/src/targets.rs) computes future forward log returns and tri-state direction targets (`+1` Long, `0` Neutral, `-1` Short) with a customizable deadband threshold.
- **Dataset Export**: [`FeatureExporter`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/features/src/export.rs) produces:
  - Versioned Parquet files for long-term analytics.
  - Arrow IPC binary streams (`.arrow`) joined strictly with forward targets, guaranteeing no unlabelled trailing lookahead rows enter the training set.

### 3.3 Python ML Training Subsystem ([`python/ml`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/python/ml))
- **Arrow IPC Loader**: [`load_arrow_training_dataset`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/python/ml/arrow_dataset.py) reads the Rust Arrow output, validates column schemas, detects anomalies, and converts to aligned NumPy buffers.
- **Model Architecture**: [`LSTMForecaster`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/python/ml/models/lstm.py) featuring:
  - Multi-layer LSTM with dropout.
  - Forget-gate bias initialization to $+1.0$ (ensuring retention of long-term dependencies at initialization).
  - Linear projection head mapping to expected continuous forward return.
- **Custom Loss Functions**: [`DirectionalAsymmetricLoss`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/python/ml/loss.py) combines Huber regression loss with asymmetric penalties on sign disagreement, heavily penalizing long predictions when actual returns are negative.
- **Cross-Validation**: [`PurgedWalkForwardSplitter`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/python/ml/dataset.py) strictly enforces purge gaps (dropping samples overlapping the label horizon) and embargo windows (preventing autoregressive leakage after training blocks).
- **Training Orchestration**: [`ModelTrainer`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/python/ml/trainer.py) and [`train_orchestrator.py`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/python/ml/train_orchestrator.py) manage the full lifecycle, generating versioned directories under `models/<model_id>/`.

### 3.4 Model Export, Parity & Rust Inference ([`quant_inference`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/inference))
- **Artifact Packaging**: [`export_to_onnx`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/python/ml/export_onnx.py) bundles dynamic batch shapes, embeds metadata (`metadata.json`), fits the training feature scaler (`scaler.json`), and executes dynamic INT8 post-training quantization (`model_int8.onnx`).
- **Numerical Parity**: Automated verification in [`test_onnx_parity.py`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/python/tests/test_onnx_parity.py) asserts that PyTorch and ONNX Runtime outputs match within $\epsilon < 10^{-5}$.
- **Rust Runtime Session**: [`OnnxSession`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/inference/src/onnx_session.rs) wraps tract-onnx for pure-Rust CPU inference.
- **Apply-Only Scaling**: [`FittedScaler`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/inference/src/scaler.rs) handles sequence normalization in Rust with zero runtime training leakage.

### 3.5 Signals, Portfolio & Risk Engines
- **Signal Processing** ([`quant_signals`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/signals)): Raw continuous predictions are calibrated into [`Signal`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/signals/src/signal.rs) instances with direction, expected return, confidence scores, deadband filters, and cross-sectional rankings.
- **Portfolio Construction** ([`quant_portfolio`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/portfolio)): [`VolatilityTargetedConstructor`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/portfolio/src/constructor.rs) computes position sizes scaled to target annualized portfolio volatility, applying strict drawdown de-risking multipliers and leverage caps.
- **Risk Evaluation** ([`quant_risk`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/risk)): [`RiskEngine`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/risk/src/engine.rs) calculates Value-at-Risk (Parametric & Historical VaR 95/99), Expected Shortfall (CVaR), running maximum drawdowns, and gross/net exposures.

### 3.6 Execution, Backtesting & Simulation
- **Execution Modeling** ([`quant_execution`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/execution)): [`CompositeExecutionModel`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/execution/src/model.rs) models real-world execution frictions: fixed commission per share, half-spread costs, square-root market impact slippage, and volume participation caps.
- **Deterministic Backtest Loop** ([`quant_backtest`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/backtest)): [`BacktestEngine`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/backtest/src/engine.rs) processes time-series bars bar-by-bar, tracks position lots, executes simulated orders, and computes equity curves, Sharpe ratio, Sortino ratio, win rate, and profit factor.
- **Monte Carlo & Regime Analysis** ([`quant_simulation`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/simulation)):
  - [`MonteCarloResampler`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/simulation/src/resampler.rs) executes stationary bootstrap resamplings over return series to derive statistical confidence bands.
  - [`RegimeDetector`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/simulation/src/regime.rs) classifies market environments into Bull, Bear, Sideways, and Crisis regimes.

---

## 4. Technical Tools, Dependencies & Environment Setup

### 4.1 System & Hardware Requirements
- **Operating System**: Windows 10/11 64-bit, Ubuntu 22.04+ LTS, or macOS (Apple Silicon/Intel).
- **CPU**: x86_64 with AVX2/FMA support (recommended for optimal ONNX Runtime CPU SIMD throughput) or ARM64 (Apple Silicon).
- **RAM**: Minimum 8 GB (16 GB recommended for running large-scale Monte Carlo resamplings and Arrow datasets).
- **C++ Runtime / Build Tools**:
  - Windows: Microsoft Visual C++ Build Tools (MSVC v143+) or Windows SDK.
  - Linux: `build-essential`, `cmake`, `pkg-config`.

### 4.2 Rust Toolchain & Dependencies
- **Rust Toolchain**: `stable` (Rust 1.70+, managed via [`rust-toolchain.toml`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust-toolchain.toml) with `clippy` and `rustfmt`).
- **Core Crates** (defined in workspace [`Cargo.toml`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/Cargo.toml)):
  - Serialization: `serde = "1.0"`, `serde_json = "1.0"`, `serde_yaml = "0.9"`
  - Time & Date: `chrono = "0.4"`
  - CLI Parsing: `clap = "4.4"` (with `derive` and `env` flags)
  - Data Formats: `arrow2 = "0.17"` (enabling `io_ipc` and `io_parquet`)
  - Numerics: `ndarray = "0.15"`
  - ML Inference: `tract-onnx = "0.21"` (pure-Rust ONNX inference)
  - Ring Buffers & Sync: `ringbuf = "0.4"`, `parking_lot = "0.12"`
  - Testing & Telemetry: `proptest = "1.4"`, `tracing = "0.1"`, `tracing-subscriber = "0.3"`

### 4.3 Python Environment & Dependencies
- **Python Version**: Python 3.10, 3.11, or 3.14 (the workspace virtual environment is configured at `c:\Users\Sahil\Downloads\lstm\.venv`).
- **Installed Packages** ([`requirements.txt`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/requirements.txt)):
  ```text
  numpy>=2.0.0
  pandas>=2.2.0
  pyarrow>=15.0.0
  pyyaml>=6.0
  scipy>=1.12.0
  torch>=2.2.0
  onnx>=1.16.0
  onnxruntime>=1.17.0
  pytest>=8.0.0
  ```

---

## 5. Complete Project Structure

```text
lstm_stock_prediction/
├── .gitignore
├── Cargo.lock
├── requirements.txt
├── rust-toolchain.toml
│
├── configs/
│   └── default.yaml                     # Unified system configuration schema
│
├── Docs/                                # Architectural & specification documents
│   ├── 00-AUDIT.md                      # Initial codebase audit & debt analysis
│   ├── 01-ARCHITECTURE.md               # Target architecture & language boundaries
│   ├── 02-DATA-FEATURES.md              # Market data & feature engine specifications
│   ├── 03-MODEL-TRAINING.md             # Model training, loss functions & walk-forward CV
│   ├── 04-INFERENCE.md                  # Rust ONNX Runtime inference core
│   ├── 05-PORTFOLIO-RISK-EXECUTION.md   # Signal -> Portfolio -> Risk -> Execution
│   ├── 06-BACKTEST-SIMULATION.md        # Event-driven backtesting & Monte Carlo specs
│   ├── 07-PERFORMANCE-TESTING.md        # CPU optimization & testing strategy
│   ├── 08-DEPLOYMENT-CONFIG.md          # Deployment, telemetry & failure recovery
│   ├── 09-DERIVATIVES-ROADMAP.md        # Options & futures roadmap
│   ├── 10-MIGRATION-PLAN.md             # Staged migration blueprint
│   ├── 11-IMPLEMENTATION-PLAN.md        # Active phase tracking backlog
│   ├── 12-IMPLEMENTATION-CHANGELOG.md   # Detailed engineering changelog & commit ledger
│   └── HANDOVER.md                      # This handover document
│
├── datasets/                            # Versioned data artifacts (git-ignored)
│   ├── market/                          # Raw OHLCV bars + manifests (.json)
│   ├── features/                        # Computed feature tables (.parquet)
│   └── training/                        # Causal training matrices (.arrow IPC)
│
├── models/                              # Versioned model artifacts
│   └── lstm_v1/
│       ├── metadata.json                # Hyperparameters, input features, commit ID
│       ├── model.onnx                   # FP32 ONNX graph
│       ├── model.pt                     # PyTorch checkpoint weights
│       ├── model_int8.onnx              # INT8 dynamically quantized model
│       ├── scaler.json                  # Fitted feature means & standard deviations
│       └── training_log.json            # Train/validation loss trajectories & metrics
│
├── python/                              # Python Machine Learning Subsystem
│   ├── ml/
│   │   ├── arrow_dataset.py             # Rust Arrow IPC reader & validation contract
│   │   ├── data_fetch_helper.py         # Subprocess bridge for data ingestion
│   │   ├── dataset.py                   # Purged walk-forward CV splitter & scaling
│   │   ├── evaluate.py                  # IC, Rank IC, Sharpe & directional accuracy
│   │   ├── export_onnx.py               # ONNX exporter with metadata embedding
│   │   ├── loss.py                      # DirectionalAsymmetricLoss & SharpeAwareLoss
│   │   ├── quantize.py                  # INT8 dynamic quantization script
│   │   ├── trainer.py                   # ModelTrainer (early stopping, checkpoints)
│   │   ├── train_orchestrator.py        # End-to-end training orchestrator entrypoint
│   │   ├── walk_forward.py              # Rolling & expanding window splits
│   │   └── models/
│   │       └── lstm.py                  # PyTorch LSTMForecaster model definition
│   ├── research/
│   │   └── legacy_v1/                   # Isolated historical Python prototype
│   └── tests/
│       ├── test_arrow_dataset.py        # Arrow IPC contract validation tests
│       ├── test_dataset_leakage.py      # Temporal leakage & purge/embargo tests
│       └── test_onnx_parity.py          # PyTorch vs ONNX Runtime numerical parity
│
├── reports/                             # Generated backtest & simulation reports
│
└── rust/                                # Rust Core Production Workspace
    ├── Cargo.toml                       # Workspace manifest
    │
    ├── backtest/                        # quant_backtest crate
    │   └── src/
    │       ├── engine.rs                # Deterministic event-driven backtest loop
    │       ├── lib.rs
    │       └── report.rs                # PnL, equity curve & metric calculations
    │
    ├── calendar/                        # quant_calendar crate
    │   └── src/
    │       ├── lib.rs
    │       └── us_equity.rs             # NYSE/NASDAQ trading calendar & market hours
    │
    ├── cli/                             # quantctl CLI crate
    │   ├── Cargo.toml
    │   ├── src/
    │   │   ├── commands.rs              # Clap CLI subcommand definitions
    │   │   ├── main.rs                  # CLI entrypoint
    │   │   ├── telemetry.rs             # Tracing initialization (JSON / stdout)
    │   │   └── handlers/
    │   │       ├── backtest.rs          # `quantctl backtest` handler
    │   │       ├── config.rs            # `quantctl config show/validate` handler
    │   │       ├── data.rs              # `quantctl data fetch/validate` handler
    │   │       ├── env.rs               # `quantctl env` handler
    │   │       ├── export_model.rs      # `quantctl export-model` handler
    │   │       ├── features.rs          # `quantctl features build` handler
    │   │       ├── predict.rs           # `quantctl predict` handler
    │   │       ├── report.rs            # `quantctl report` handler
    │   │       ├── simulate.rs          # `quantctl simulate` handler
    │   │       └── train.rs             # `quantctl train` handler (spawns Python ML)
    │   └── tests/
    │       └── cli_tests.rs             # End-to-end integration tests for all commands
    │
    ├── config/                          # quant_config crate
    │   └── src/
    │       ├── lib.rs
    │       ├── loader.rs                # YAML + Environment variable loader
    │       ├── types.rs                 # Strongly typed configuration structs
    │       └── validation.rs            # Startup constraint & boundary validation
    │
    ├── data/                            # quant_data crate
    │   └── src/
    │       ├── adjust.rs                # Split & dividend corporate action adjustment
    │       ├── lib.rs
    │       ├── provider.rs              # MarketDataProvider trait
    │       ├── storage.rs               # Versioned dataset storage & manifest I/O
    │       ├── types.rs                 # Bar, Tick, OHLCV data structures
    │       ├── validate.rs              # Monotonicity & candle validity rules
    │       └── adapters/
    │           ├── mock.rs              # Deterministic synthetic data generator
    │           └── yfinance.rs          # Yahoo Finance HTTP ingestion adapter
    │
    ├── execution/                       # quant_execution crate
    │   └── src/
    │       ├── lib.rs
    │       ├── model.rs                 # CompositeExecutionModel (spread, slip, impact)
    │       └── order.rs                 # Order, Fill, OrderType, OrderSide
    │
    ├── features/                        # quant_features crate
    │   ├── src/
    │   │   ├── atr.rs                   # Average True Range & True Range
    │   │   ├── bollinger.rs             # Bollinger Bands
    │   │   ├── export.rs                # Parquet & Arrow IPC training dataset writer
    │   │   ├── graph.rs                 # Directed acyclic feature computation graph
    │   │   ├── lib.rs
    │   │   ├── macd.rs                  # MACD line, signal & histogram
    │   │   ├── moving_avg.rs            # SMA & EMA
    │   │   ├── rsi.rs                   # Wilder's smoothed RSI
    │   │   ├── store.rs                 # In-memory & Parquet feature store
    │   │   ├── targets.rs               # Forward returns & tri-state direction targets
    │   │   ├── volatility.rs            # Rolling volatility & log returns
    │   │   └── window.rs                # Ring buffer sliding window implementation
    │   └── tests/
    │       └── indicator_golden_tests.rs# Golden tests against closed-form benchmarks
    │
    ├── inference/                       # quant_inference crate
    │   └── src/
    │       ├── lib.rs
    │       ├── metadata.rs              # Model metadata JSON deserialization
    │       ├── onnx_session.rs          # Microsoft ONNX Runtime CPU session wrapper
    │       ├── provider.rs              # PredictionProvider trait & OnnxLstmProvider
    │       └── scaler.rs                # FittedScaler apply-only normalizer
    │
    ├── instruments/                     # quant_instruments crate
    │   └── src/
    │       ├── derivative.rs            # FutureInstrument & OptionInstrument models
    │       ├── equity.rs                # EquityInstrument
    │       └── lib.rs                   # Instrument trait & AssetClass enum
    │
    ├── portfolio/                       # quant_portfolio crate
    │   ├── src/
    │   │   ├── constructor.rs           # Volatility-targeted portfolio constructor
    │   │   ├── lib.rs
    │   │   ├── portfolio.rs             # Position tracking & cash ledger
    │   │   └── position.rs              # Position struct
    │   └── tests/
    │       └── stage5_integration_tests.rs # End-to-end signal-to-portfolio pipeline
    │
    ├── risk/                            # quant_risk crate
    │   └── src/
    │       ├── engine.rs                # VaR, CVaR & max drawdown calculators
    │       ├── lib.rs
    │       └── limits.rs                # Gross/net exposure & concentration limits
    │
    ├── signals/                         # quant_signals crate
    │   └── src/
    │       ├── lib.rs
    │       ├── signal.rs                # Signal struct & calibration
    │       ├── stream.rs                # SignalStream trait
    │       └── transforms.rs            # Deadband, ranking & confidence transforms
    │
    └── simulation/                      # quant_simulation crate
        └── src/
            ├── lib.rs
            ├── regime.rs                # Market regime classification
            └── resampler.rs             # Monte Carlo stationary bootstrap
```

---

## 6. Operational Runbook & Execution Guide

All platform interactions run through the unified Rust CLI binary: `quantctl`.

### 6.1 Step-by-Step Production Pipeline

#### Step 1: Ingest & Validate Market Data
```powershell
# Ingest historical data for AAPL
cargo run --bin quantctl -- data fetch --symbol AAPL --start 2015-01-01 --end 2024-01-01 --version ds_2024_v1

# Validate integrity, monotonicity, and candle sanity
cargo run --bin quantctl -- data validate --dataset ds_2024_v1 --symbol AAPL
```

#### Step 2: Build Features & Arrow IPC Training Dataset
```powershell
# Computes 9 technical indicators, forward targets, and exports versioned Arrow IPC dataset
cargo run --bin quantctl -- features build --symbol AAPL --dataset ds_2024_v1 --feature-set baseline_v1
```
*Output Artifacts:*
- `datasets/features/baseline_v1/v1/AAPL.parquet` + `.manifest.json`
- `datasets/training/ds_2024_v1/AAPL.arrow` + `.manifest.json`

#### Step 3: Train Model in PyTorch & Export ONNX
```powershell
# Executes PyTorch training on the Arrow dataset and produces the versioned ONNX artifact
cargo run --bin quantctl -- train --dataset ds_2024_v1 --symbol AAPL --epochs 50 --batch-size 64
```
*Output Artifacts:*
- `models/lstm_v1/model.onnx`
- `models/lstm_v1/model_int8.onnx`
- `models/lstm_v1/metadata.json`
- `models/lstm_v1/scaler.json`
- `models/lstm_v1/training_log.json`

#### Step 4: Run Native Rust Inference
```powershell
# Runs zero-copy ONNX inference in Rust using Microsoft ONNX Runtime
cargo run --bin quantctl -- predict --symbol AAPL --model lstm_v1
```

#### Step 5: Run Deterministic Event-Driven Backtest
```powershell
# Runs bar-by-bar backtest applying spread, slippage, volatility targeting, and risk limits
cargo run --bin quantctl -- backtest --symbol AAPL --start 2023-01-01 --end 2024-01-01
```

#### Step 6: Run Monte Carlo Simulation & Regime Analysis
```powershell
# Resamples historical returns across 1,000 bootstrap runs and detects market regimes
cargo run --bin quantctl -- simulate --symbol AAPL --runs 1000
```

#### Step 7: Generate Performance Report
```powershell
# Emits summary analytics, Sharpe ratio, Sortino, drawdowns, and VaR/CVaR
cargo run --bin quantctl -- report --symbol AAPL
```

---

## 7. Verification, Testing & Quality Gates

The platform enforces strict automated testing gates across both languages:

### 7.1 Running the Rust Test Suite
```powershell
cd rust
cargo test --workspace
```
*Current Results:* **All tests pass** (70+ unit, integration, and golden tests):
- `quant_features`: 28 unit tests, 4 closed-form golden tests (SMA, EMA, RSI Wilder, Bollinger).
- `quant_inference`: 9 unit tests verifying ONNX session loading, prediction, and scaler transforms.
- `quant_data`: 7 unit tests, 4 property-based tests asserting monotonicity and rejection of corrupt ticks.
- `quant_portfolio`: 4 unit tests, 1 full pipeline integration test.
- `quant_signals`: 8 unit tests (calibration, transforms, confidence).
- `quant_simulation`: 3 unit tests (regime classifier, Monte Carlo distributions).
- `quantctl`: 9 CLI integration tests (`test_cli_train`, `test_cli_predict`, `test_cli_backtest`, etc.).

### 7.2 Running the Python Test Suite
```powershell
# Using the workspace virtual environment:
c:\Users\Sahil\Downloads\lstm\.venv\Scripts\python.exe -m pytest python/tests -v
```
*Current Results:* **11 passed in 21s**:
- `test_arrow_dataset.py` (2 passed): Asserts Arrow IPC schema compliance, missing-column rejection, and timestamp monotonicity.
- `test_dataset_leakage.py` (3 passed): Asserts zero temporal leakage across train/validation splits, verification of purge/embargo windows, and scaler fit boundaries.
- `test_onnx_parity.py` (6 passed): Asserts numerical parity between PyTorch and ONNX Runtime CPU across batch sizes (1, 4, 16) and sequence lengths (32, 64) within $\epsilon < 10^{-5}$.

---

## 8. Current Working Tree Status & Immediate Handover Next Steps

### 8.1 Working Tree Status
- The active git branch is `main`, clean at commit `3dcca43`.
- Local working tree contains refinements across `rust/` crates and `models/lstm_v1/` that compile cleanly and pass the full test suite.
- Generated datasets and test outputs under `datasets/` and `reports/` are intentionally git-ignored.

### 8.2 Prioritized Roadmap for the Next Engineer
1. **Model Signal Stream Integration**:
   - Currently, `quantctl backtest` uses calibrated rules and model test fixtures. Wire [`ModelSignalStream<OnnxLstmProvider>`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/signals/src/stream.rs) directly into the backtest loop so live predictions drive simulated portfolio rebalancing.
2. **Streaming Feature Pipeline in Predict**:
   - Update `quantctl predict` to query historical bars from [`quant_data::storage`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/data/src/storage.rs), feed them through [`FeatureGraph`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/features/src/graph.rs) to construct the lookback tensor, and pass that dynamically into `OnnxLstmProvider`.
3. **Walk-Forward Retraining Loop**:
   - Expand `train_orchestrator.py` with multi-fold purged cross-validation to retrain quarterly and save rolling model artifacts.
4. **CI/CD Pipeline**:
   - Add `.github/workflows/ci.yml` running `cargo test --workspace`, `cargo clippy`, `rustfmt --check`, and `pytest python/tests` on every pull request.
5. **Derivatives Pricing Engine (Phase 10 Extension)**:
   - Wire the Black-Scholes Greeks calculators in [`quant_instruments::derivative`](file:///c:/Users/Sahil/Downloads/lstm/lstm_stock_prediction/rust/instruments/src/derivative.rs) into the risk engine for Delta/Gamma exposure hedging.

---
*Document prepared for technical handover and institutional audit.*

## 2026-10-13 global data continuation

Continue from Docs/BUILD_PLAN.md and Docs/23-GLOBAL-MARKET-FORECASTS.md. Twelve Data
history ingestion and fresh forecast mode are implemented and locally verified.
Market identity flows from config through Rust Arrow manifests into sealed Python
model metadata. Fresh mode rejects incompatible/demo models, unknown/missing
volume, stale bars, credential errors and quota exhaustion. It uses process-local
shared cache/budget controls and preserves immutable source datasets. All tests
listed in BUILD_PLAN passed. Working tree contains the implementation; no commit
or push was made. User wants preparation around missing credentials, and previously
chose to deploy later. Do not claim live account verification or predictive quality.
