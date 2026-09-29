# Quant research inspector

A local Rust/PyTorch quantitative research platform with an embedded React inspector. Build validated datasets, train versioned models, replay held-out predictions with simulated execution costs, and inspect the resulting evidence.

Rust owns features, CPU inference through **tract-onnx**, portfolio accounting, backtesting, bootstrap simulation and the localhost API. Python owns training and export. Node is a build dependency only. This is a research simulator; it does not place live orders. Synthetic demo results are labelled.

## Documentation

- [Live capture, fresh research and forecast runbook](Docs/19-LIVE-INGESTION.md)
- [Production checklist and release gates](Docs/20-PRODUCTION-CHECKLIST.md)
- [Forecast website and public deployment scope](Docs/21-WEB-FORECAST-DEPLOYMENT.md)
- [System audit, offline setup, required inputs and remaining work](SYSTEM_AUDIT_AND_OFFLINE_GUIDE.md)
- [Complete capability breakdown](Docs/14-CAPABILITIES.md)
- [Software-only and hardware-aware improvement roadmap](Docs/15-IMPROVEMENTS.md)
- [Verification results](Docs/16-VERIFICATION.md)
- [Artifact and API contract](Docs/13-ARTIFACT-API.md)
- [Original implementation plan](Docs/11-IMPLEMENTATION-PLAN.md)
- [Frontend design](web/DESIGN.md)

## Setup

Use current stable Rust with a native compiler toolchain, Python 3.14, uv, and Node 22. Windows requires MSVC build tools. Run from the repository root:

```powershell
cd python
uv sync --locked
cd ../web
npm ci
npm run build
cd ..
cargo build --manifest-path rust/Cargo.toml --bin quantctl --release
```

Build web assets **before** compiling Rust. A backend-only build serves a setup page and the API. Rebuild Rust after changing the SPA. The compiled application requires no Node process.

Open `/forecast` on the running inspector to generate an ONNX forecast from a
selected model and recorded dataset. The result includes the actual data timestamp,
horizon and provenance; historical and synthetic inputs are explicit. The live
price stream is not yet connected to the forecast feature window. Public hosting
and authenticated access remain deployment gates; the default server is local.

## Reproducible demo

The demo explicitly selects synthetic data and a small two-epoch model. It exercises the real pipeline; its results are not evidence of trading performance.

```powershell
$quantctl = './rust/target/release/quantctl.exe'
& $quantctl --config configs/demo.yaml data ingest
& $quantctl --config configs/demo.yaml data validate
& $quantctl --config configs/demo.yaml features build
& $quantctl --config configs/demo.yaml train
& $quantctl --config configs/demo.yaml predict --model inspector_demo --symbol AAPL
& $quantctl --config configs/demo.yaml backtest run --model inspector_demo --split test
& $quantctl serve --root . --port 8787
```

On Linux/macOS use `./rust/target/release/quantctl` without `.exe`. Open [the local inspector](http://127.0.0.1:8787).

The backtest prints a run ID. Preserve provenance by simulating its immutable report:

```text
quantctl simulate --report reports/runs/<run-id>/report.json --paths 500
```

Enable the explicit in-sample/unverified toggle to inspect the new simulation. A bootstrap scenario is not a newly verified OOS forecast.

Training reserves the model ID and exports under `models/.staging`. Only a complete package with passing ONNX parity checks is renamed into the final model directory. Failed attempts clean up automatically; interrupted processes may leave hidden stages and locks that need inspection after the process has stopped. Choose a new `training.model_id` for a new experiment; existing models cannot be overwritten. Test reuse requires deliberate `--allow-reuse`. Run commands from one repository root so artifact paths remain consistent.

## Market data and models

Edit a copy of `configs/default.yaml` with distinct dataset/model versions and the desired date range. The default `csv` provider reads `datasets/import/<symbol>.csv` (or `data.input_dir`). Use the exact header `timestamp,open,high,low,close,volume`, RFC3339 timestamps with timezones, strictly increasing bars, valid OHLC prices and integer volume. Dates select UTC `[start_date, end_date)`. Ingest before building features; an existing market dataset version cannot be overwritten. Feature building processes configured symbols; training/backtest require exactly one symbol per run.

Operation is offline after the one-time dependency setup. Yahoo acquisition is disabled unless `QUANTCTL_ALLOW_NETWORK=1` is explicitly set on a connected acquisition machine; it additionally requires the optional Python `yfinance` dependency. Failures never fall back to synthetic data. The [offline guide](SYSTEM_AUDIT_AND_OFFLINE_GUIDE.md) covers preparation, CSV inputs, checks and the distinction between application policy and machine-level isolation.

Legacy `lstm_v1` metadata references synthetic feature names. Retrain on the Rust Arrow dataset; unavailable model features are rejected, never replaced with fabricated values.

New training runs publish `integrity.json` with SHA-256 hashes for every model-package file. Rust verifies the manifest before prediction or backtesting. Missing manifests and modified files are rejected; retrain older models to produce a verified package. Hashes detect accidental changes relative to the manifest, not deliberate replacement of the manifest itself.

## Inspector

Overview, Backtest, Data and validation, Models, Signals, Risk and simulation, and Run activity share a run selector and copyable provenance footer. Rust supplies metrics, Arrow series and split records. The browser formats and plots them without recomputing financial metrics. Unsupported capabilities have explicit empty states. Desktop/mobile layouts, keyboard focus, accessible tables and browser tests are included.

The server binds only to `127.0.0.1`. It has no account system, remote deployment or job-launching API. Published run files are immutable resources.

## Verification and development

```powershell
cargo fmt --manifest-path rust/Cargo.toml --all -- --check
cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings
cargo test --manifest-path rust/Cargo.toml --workspace
python/.venv/Scripts/python.exe -m pytest python/tests -q
cd web
npm run contract:check
npm test
npm run build
npx playwright install chromium
npm run test:e2e
```

`scripts/verify.ps1` runs the main checks on Windows using installed dependencies and offline Cargo resolution. `-Benchmarks` also runs benchmark smoke checks. Dependency installation is opt-in via `-InstallDependencies`; install Python and Playwright Chromium during connected setup. `python/.venv/Scripts/python.exe scripts/offline_doctor.py` checks local readiness. CLI integration tests perform real small-model training and create uniquely versioned, git-ignored artifacts. They require the Python environment but no live market downloads.

For frontend development, serve Rust on port 8787 and run `npm run dev` in `web`. Vite proxies `/api`. After API edits run `npm run contract` and commit generated TypeScript and `Docs/openapi.json`.

```text
cargo bench --manifest-path rust/Cargo.toml --bench feature_benchmarks
cargo bench --manifest-path rust/Cargo.toml --bench simulation_benchmarks
```

Compare benchmarks on the same machine and workload. Compilation and smoke tests do not establish a latency SLA.

## Scope

Advanced libraries such as multi-horizon training, conformal calibration, Kelly/sector-neutral optimization, nonlinear impact, regime overlays and derivatives are not all selected by the default CLI. Full multi-asset replay and research inspector datasets remain future work. The capability guide distinguishes operational integration from library availability and lists statistical limitations.

License: MIT OR Apache-2.0.

## Rolling evaluation

Use `quantctl --config configs/demo.yaml train --walk-forward --folds 3` with a new model ID and enough rows for every train/validation/test window. Each fold has an exported model and Rust parity evidence. The root model is the last fold; `backtest run` replays that fold, while pooled prediction metrics are recorded separately in `validation.json`. New backtests also supply benchmark, positions, realized signal outcomes and terminal risk evidence to the inspector.
