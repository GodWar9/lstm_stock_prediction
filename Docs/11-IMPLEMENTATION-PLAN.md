> Current behavior: see the [capability guide](14-CAPABILITIES.md) and [verification report](16-VERIFICATION.md). Historical checkboxes and latency targets below do not establish CLI integration or measured SLA compliance.

# 11 - Implementation Plan

## Objective

Build a reproducible quantitative research platform, not only an LSTM demo:

```text
market data -> PIT validation -> Rust features -> Arrow dataset
-> Python training -> versioned ONNX artifact -> Rust inference
-> signals -> portfolio/risk/execution -> backtest -> simulation/report
```

Each phase must leave the repository runnable and tested. Commits should be
small, coherent, and tied to a behavior or verification change.

The detailed record of completed changes, verification, commit IDs, and
remaining work is maintained in
[12-IMPLEMENTATION-CHANGELOG.md](./12-IMPLEMENTATION-CHANGELOG.md).

## Phases

| Phase | Scope | Status |
|---|---|---|
| 1 | Foundation, configuration, CI, smoke tests | Complete |
| 2 | Versioned market-data ingestion | Complete |
| 3 | Point-in-time and leakage validation | Complete |
| 4 | Rust feature store and Arrow dataset builder | Complete |
| 5 | Python training contract and reproducibility | Complete |
| 6 | Versioned artifacts and Rust inference parity | Complete |
| 7 | Real prediction and model-backed signals | Complete |
| 8 | Portfolio, risk, execution, and deterministic backtesting | Complete |
| 9 | Monte Carlo, stress testing, benchmarks, and observability | Complete |
| 10 | Documentation, deployment polish, uv migration, and CI | Complete |

## Completed Vertical Slices

### Slice 1: Market Data → Feature Store → Arrow Training Dataset

- Versioned dataset paths under `datasets/market/<dataset-version>/`
- Serialized bars and dataset manifests
- Strict OHLCV validation before persistence
- Duplicate timestamp rejection
- Persisted dataset reload and validation through `quantctl data validate`
- Explicit synthetic provider behavior rather than silent fallback
- Versioned Arrow IPC training dataset joining feature rows with forward targets
- Python schema- and manifest-aware reader for the Arrow contract
- Training orchestrator consuming Arrow datasets by default with `--synthetic` flag

### Slice 2: Training → Inference → Signal → Portfolio → Backtest

- PyTorch LSTMForecaster with forget gate initialization and directional loss
- Purged/embargoed walk-forward cross-validation with scaler isolation
- ONNX export with INT8 dynamic quantization and numerical parity verification
- Rust ONNX Runtime CPU inference via `ort` crate
- Signal calibration with deadband filtering and cross-sectional ranking
- Volatility-targeted portfolio construction with drawdown de-risking
- Execution simulation with spread, slippage, and market impact models
- Event-driven backtesting engine with PnL, Sharpe, Sortino, and drawdowns
- Monte Carlo stationary bootstrap resampling and market regime classification

### Slice 3: Platform Polish and Migration

- Python dependency management migrated from pip/requirements.txt to uv
- `python/pyproject.toml` with locked dependencies in `python/uv.lock`
- `quantctl train` subprocess updated to resolve `python/.venv` interpreter
- 120 meaningful commits across implementation, tests, and documentation
- Comprehensive README with architecture diagrams and operational runbook
- GitHub CI pipeline for Rust workspace tests, clippy, and Python test suite

## Commit Standard

Prefer commits such as:

```text
feat(data): persist versioned OHLCV datasets
test(data): reject duplicate timestamps in persisted data
feat(features): build Arrow training dataset from validated bars
test(training): enforce scaler fit boundary
```

The commit target of 100+ meaningful commits has been exceeded with 120 commits
across implementation, tests, benchmarks, documentation, and reproducibility
work.
