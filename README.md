# Quant Research Inspector

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)
[Live demo](https://lstm-research.vercel.app)

A Rust/PyTorch quantitative research platform for **leakage-safe, reproducible** model research. Build validated datasets, train versioned LSTM models, replay held-out predictions through a cost-aware backtester, stress-test results with bootstrap simulation, and inspect every run in a React UI.

> **Research simulator only.** It does not place live orders. Synthetic demo results are always labelled as such and are not evidence of trading performance.

## Why this exists

Most LSTM stock demos leak future data, skip transaction costs, and can't be reproduced. This project treats the *evidence chain* as the product:

- **Point-in-time correctness** — leakage/purge-embargo validation on every dataset
- **Immutable, hashed artifacts** — datasets, models and reports carry SHA-256 provenance
- **Parity-gated models** — a model is only published if PyTorch, ONNX Runtime and tract agree
- **Honest scope** — unsupported capabilities show explicit empty states, never fabricated numbers

## Architecture

| Layer | Owner | Responsibility |
|---|---|---|
| Data, features, Arrow IPC | **Rust** | Ingestion, validation, feature store |
| Inference | **Rust** (`tract-onnx`) | CPU ONNX inference, no Python at serve time |
| Portfolio, risk, execution, backtest, Monte Carlo | **Rust** | Cost-aware replay, bootstrap + regime analysis |
| API + CLI (`quantctl`) | **Rust** | Serves the inspector and artifacts |
| Training + ONNX export | **Python** (PyTorch, uv) | The only Python in the loop |
| Inspector UI | **React/Vite** (`web/`) | Plots Rust-computed metrics; never recomputes them |

Local serving needs no Node process. The Vercel deployment uses a thin Node API bridge to a Rust backend.

## Status

| Area | State |
|---|---|
| Versioned ingestion, validation, leakage checks | ✅ Done |
| Rust feature store + Arrow IPC | ✅ Done |
| PyTorch training, ONNX export, 3-way parity gate | ✅ Done |
| Integrity manifests (`integrity.json`, SHA-256) | ✅ Done |
| Walk-forward training (`--walk-forward --folds N`) | ✅ Done |
| Backtester, Monte Carlo, regime analysis | ✅ Done |
| React inspector + `/forecast` page | ✅ Done |
| CI gates (fmt, clippy, tests, pytest, benches) | ✅ Done |
| Vercel + Render free-tier deployment | ✅ Done |
| Portable Windows app (`lstm-inspector.exe`) | ✅ Done |
| Live price stream → forecast feature window | ⏳ Not connected yet |
| Multi-asset replay, full inspector datasets | ⏳ Future work |
| Multi-horizon, conformal, Kelly/sector-neutral, nonlinear impact, derivatives in default CLI | ⏳ Libraries exist; not all selected by default |

See the [capability guide](Docs/14-CAPABILITIES.md) for exactly what is wired into the default pipeline versus available as a library, and the [verification report](Docs/16-VERIFICATION.md) for measured results.

## Quick start (no Docker, no API keys)

Requires Node 22.12+, Rust (+ MSVC on Windows), Python 3.14.

```bash
cd web
npm ci
npm run demo
```

Open the printed URL. This runs **real inference** on a bundled synthetic dataset. Ctrl+C removes the temporary workspace.

**Or** download the portable `lstm-inspector.exe` — no Node, Rust, Python or Docker required. See the [portable app guide](Docs/24-PORTABLE-APP.md).

## Full setup (training)

```bash
cd python && uv sync --locked
cd ../web && npm ci && npm run build
cd .. && cargo build --manifest-path rust/Cargo.toml --bin quantctl --release
```

> Build the web assets **before** Rust. Rebuild Rust after any SPA change.

## Reproducible demo pipeline

```powershell
$q = './rust/target/release/quantctl.exe'
& $q --config configs/demo.yaml data ingest
& $q --config configs/demo.yaml data validate
& $q --config configs/demo.yaml features build
& $q --config configs/demo.yaml train
& $q --config configs/demo.yaml predict --model inspector_demo --symbol AAPL
& $q --config configs/demo.yaml backtest run --model inspector_demo --split test
& $q serve --root . --port 8787
```

Linux/macOS: drop `.exe`. Then open <http://127.0.0.1:8787>.

Stress-test a finished run (its report is immutable):

```bash
quantctl simulate --report reports/runs/<run-id>/report.json --paths 500
```

Rolling evaluation:

```bash
quantctl --config configs/demo.yaml train --walk-forward --folds 3
```

## Data

- **Default:** CSV import from `datasets/import/<symbol>.csv` with header `timestamp,open,high,low,close,volume` (RFC3339 timestamps with timezone, strictly increasing, valid OHLC, integer volume).
- **Fresh global forecasts:** Twelve Data stock/ETF ingestion — see [Docs/23](Docs/23-GLOBAL-MARKET-FORECASTS.md).
- **Offline by default.** Yahoo acquisition requires `QUANTCTL_ALLOW_NETWORK=1` plus the optional `yfinance` dependency. Failures never fall back to synthetic data.
- Existing dataset or model versions are never overwritten; choose a new `training.model_id` per experiment.

## Model integrity

Training stages into `models/.staging` and only renames a package into place after ONNX parity passes. Each package ships `integrity.json`; Rust verifies it before predict/backtest. Missing manifests or modified files are rejected. Hashes detect accidental change, not deliberate replacement of the manifest itself.

> Legacy `lstm_v1` metadata references synthetic feature names — retrain on the Rust Arrow dataset.

## Verification

```bash
cargo fmt    --manifest-path rust/Cargo.toml --all -- --check
cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings
cargo test   --manifest-path rust/Cargo.toml --workspace
python -m pytest python/tests -q
cd web && npm run contract:check && npm test && npm run build && npm run test:e2e
```

Windows shortcut: `scripts/verify.ps1` (`-Benchmarks`, `-InstallDependencies`). Readiness check: `python scripts/offline_doctor.py`.

Benchmarks: `cargo bench --manifest-path rust/Cargo.toml --bench feature_benchmarks` (and `simulation_benchmarks`). Compare on the same machine; smoke tests do not establish a latency SLA.

## Security

Server binds to `127.0.0.1` by default. Remote mode requires a backend token. No visitor accounts, no job-launching API; published run files are immutable.

## Documentation

- [Capabilities](Docs/14-CAPABILITIES.md) · [Verification](Docs/16-VERIFICATION.md) · [Improvements roadmap](Docs/15-IMPROVEMENTS.md)
- [Artifact & API contract](Docs/13-ARTIFACT-API.md) · [Frontend design](web/DESIGN.md)
- [Deployment](Docs/22-DEPLOYMENT.md) · [Vercel website](Docs/26-VERCEL-WEBSITE.md) · [Live ingestion](Docs/19-LIVE-INGESTION.md)
- [Production checklist](Docs/20-PRODUCTION-CHECKLIST.md) · [System audit & offline guide](SYSTEM_AUDIT_AND_OFFLINE_GUIDE.md)

## License

MIT OR Apache-2.0
