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
| 2 | Versioned market-data ingestion | In progress |
| 3 | Point-in-time and leakage validation | In progress |
| 4 | Rust feature store and Arrow dataset builder | Complete |
| 5 | Python training contract and reproducibility | In progress |
| 6 | Versioned artifacts and Rust inference parity | Planned |
| 7 | Real prediction and model-backed signals | Planned |
| 8 | Portfolio, risk, execution, and deterministic backtesting | Planned |
| 9 | Monte Carlo, stress testing, benchmarks, and observability | Planned |
| 10 | Documentation, deployment polish, and optional derivatives | Planned |

## Current Vertical Slice

The first implementation slice provides:

- versioned dataset paths under `datasets/market/<dataset-version>/`
- serialized bars and dataset manifests
- strict OHLCV validation before persistence
- duplicate timestamp rejection
- persisted dataset reload and validation through `quantctl data validate`
- explicit synthetic provider behavior rather than silent fallback

The current slice produces a versioned Arrow IPC training dataset by joining
feature rows with forward targets. Python now has a schema- and manifest-aware
reader for this contract. The training orchestrator consumes that dataset by
default, derives its feature schema and dimensions from the Arrow contract, and
requires an explicit `--synthetic` flag for development-only synthetic runs.

## Commit Standard

Prefer commits such as:

```text
feat(data): persist versioned OHLCV datasets
test(data): reject duplicate timestamps in persisted data
feat(features): build Arrow training dataset from validated bars
test(training): enforce scaler fit boundary
```

Do not create artificial commits solely to increase the count. The target is
more than 100 meaningful commits across implementation, tests, benchmarks,
documentation, and reproducibility work.
