# Full-stack verification — 2026-11-01

Verification date: 2026-11-01. Commit: `67a74c3` (Deploy research website as
one authenticated Vercel container). Environment: Windows, Rust 1.96.0,
Node 24.12.0, Python 3.14.4. All commands ran from the repository root.
CI uses Ubuntu and Node 22; local results do not establish a remote CI pass.

## Results summary

| Check | Command | Result |
|---|---|---|
| Rust formatting | `cargo fmt --all -- --check` | ✅ Pass |
| Clippy | `cargo clippy --workspace --all-targets -- -D warnings` | ✅ Pass (0 warnings) |
| Rust tests | `cargo test --workspace` | ✅ **194 passed**, 0 failed (35 suites, ~34 s) |
| Python tests | `pytest python/tests -q` | ✅ **56 passed**, 39 warnings in 42 s |
| Contract check | `npm run contract:check` | ✅ Pass |
| Web unit tests | `npm test` (vitest) | ✅ **13 passed** (2 files, 1.2 s) |
| Production build | `npm run build` (tsc + vite) | ✅ 324 modules, 189 kB gzipped, 4 s |
| Browser e2e | `npm run test:e2e` (Playwright) | ✅ **52 passed** (desktop + mobile, 46.9 s) |

**Totals: 315 tests passed, 0 failed.**

## Rust workspace breakdown (194 tests)

- `quant_api`: 20 tests (2.0 s) — API pagination, filters, cancellation
- `quant_backtest`: 3 tests — replay, deterministic fills
- `quant_calendar`: 3 tests — trading calendar
- `quant_config`: 3 tests — config validation
- `quant_data`: 1 test — data pipeline
- `quant_execution`: 6 unit + 1 doc-test — execution realism, scratch allocator
- `quant_features`: 9 unit + 19 integration + 4 golden + 53 export + 1 doc-test — indicators, IPC, feature store
- `quant_inference`: 12 tests (1.0 s) — ONNX/tract providers, scaler, integrity
- `quant_instruments`: 4 tests — equity, derivatives
- `quant_portfolio`: 9 tests — Kelly, QP optimiser, sector classification
- `quant_risk`: 1 test — risk engine
- `quant_signals`: 8 tests — signal calibration, transforms
- `quant_simulation`: 6 tests — guardrails, regime detection, Monte Carlo
- `lstm_inspector`: 4 tests — bundle extraction, path traversal
- `quantctl` (CLI integration): 10 tests (30 s) — full pipeline end-to-end

## Python test details (56 tests, 39 warnings)

Warnings are PyTorch ONNX legacy exporter deprecation, tracer conversion and
batch-size advisories. All relate to `torch.onnx.export` internals and do not
affect correctness. Key suites: ONNX numerical parity (6 configurations × 2
batch sizes), walk-forward orchestration (3 folds with ONNX + tract export).

## Browser e2e details (52 tests)

Playwright against the embedded Rust server (`quantctl`) serving the production
Vite build. Desktop (Chromium) and mobile viewports tested in parallel across
6 workers. Covers: inspector routes, run/backtest/data/model/signal/risk views,
forecast page (empty datasets, incompatible intervals, forecast evidence,
credential errors, provider failures), live price feed (SSE freshness,
disconnect, reconnect, stale expiry, entitlement errors), audit table
pagination, run filter persistence, chart rendering and simulation toggle.

## What was NOT verified

- Remote CI (GitHub Actions) run — not triggered from local machine.
- Docker container build and `container_smoke.py` — requires Docker daemon.
- Benchmark execution gate (`cargo bench`) — not run to save time.
- Live market data download (Twelve Data / Alpaca) — no API keys configured.
- Deployment to Vercel / Render — not exercised.
- Numeric performance regression thresholds or latency SLA.
- Live order placement (out of scope by design).

---

# Engineering follow-up verification - 2026-09-27

Complete audit-table pagination, bounded artifact workers and working-source
provenance pass 156 Rust tests, 42 Python tests (39 ONNX-related warnings), four
frontend units and 26 desktop/mobile browser checks. Workspace Clippy, formatting,
production frontend build and generated API-contract checks pass.

New API tests reconstruct all 2,505 records across pages, including tied timestamps,
filter before counting, reject invalid pagination and oversized JSON, and preserve
worker permits after request cancellation. Source tests detect modified, untracked
and deleted code. Browser tests traverse 205 test records on desktop and mobile.
Training and replay integration tests verify stored source fingerprints.

An initial browser startup encountered a Windows executable lock while Python
parity checks were active; the successful run followed their completion. No numeric
performance SLA, production concurrency/RSS benchmark or remote CI result is claimed.

---

# Current-product completion verification — 2026-09-27

The resumed implementation passes 153 Rust tests (including documentation tests),
42 Python tests, 4 frontend unit tests and 24 desktop/mobile Playwright checks.
Rust formatting, workspace/all-target Clippy, generated API-contract checks and
the production frontend build pass. Python reports 39 existing ONNX exporter,
tracing and deprecation warnings.

New regression coverage verifies three independently exported walk-forward folds,
train-only scaler statistics, separated train/validation/test boundaries, embargo
application, per-fold ONNX/tract parity, benchmark return alignment and the new
benchmark/position/signal-outcome/risk artifacts. Browser fixtures include an actual
synthetic CLI backtest. Desktop and mobile risk screenshots were visually checked.
CI now builds the Rust verifier before running Python orchestration tests.

Python execution, esbuild and browser launching required runs outside the Windows
sandbox; the results above come from successful reruns. No remote CI result,
live-market download, pooled walk-forward trading replay, live orders or performance
SLA is claimed. Root walk-forward inference/replay uses the final fold, while pooled
out-of-sample prediction metrics are saved separately.

---

# Verification report

Verification date: 2026-09-26. Environment: Windows, Rust 1.96.0,
Node 24.12.0, Python 3.14.4. Commands ran from the repository root unless
otherwise stated. CI uses Ubuntu and Node 22; local results do not establish
that a remote CI run has passed.

## Model integrity follow-up

New packages now include SHA-256 manifests, verified before Rust provider loading.
Follow-up checks on 2026-09-26:

- Python suite: **39 passed**, including independent recomputation of all six
  published file hashes and byte counts.
- Rust inference/CLI suites: **18 passed**. The inference suite retains two
  conditional legacy-artifact checks; the CLI integration always trains and
  loads a fresh sealed model, predicts, backtests and simulates, then changes
  the scaler bytes and verifies that subsequent prediction fails.
- Dedicated integrity tests reject same-size mutations, missing package members,
  absent manifests, incomplete/extra file entries, unsupported schemas/algorithms
  and invalid digests.
- Clippy passed for all inference and CLI targets; Rust formatting completed.

No new claim is made about market/training data integrity, manifest authenticity,
or concurrent manual edits after verification. Legacy unsealed packages require
retraining for prediction/backtesting; historical inspector fixtures remain readable.

## Atomic model publication follow-up

The training orchestrator now reserves IDs before training and publishes only
complete, validated model directories. Follow-up checks on 2026-09-26:

- Python suite: **39 passed**, with the same 30 ONNX exporter/deprecation warnings.
- Rust CLI integration suite: **6 passed**, including real training, complete
  package checks, staging cleanup, overwrite rejection, inference and replay.
- Rust formatting and Clippy for the CLI integration target passed. The existing
  browser/API contract is unchanged.

New tests exercise failed exports, missing files, malformed JSON, wrong model
IDs, nonfinite/failed parity, invalid paths, simultaneous ID reservations,
destination collisions and success reporting after publication. Abrupt process
termination can leave a hidden stage/lock; automatic stale-lock deletion and
power-loss durability are not claimed.

## Baseline checks before the publication follow-up

| Check | Command | Result |
|---|---|---|
| Rust formatting | `cargo fmt --manifest-path rust/Cargo.toml --all -- --check` | Passed |
| Rust lint, all targets | `cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings` | Passed |
| Rust workspace tests | `cargo test --manifest-path rust/Cargo.toml --workspace` | 149 passed; 0 failed; 0 ignored |
| Python units and ONNX parity | `python/.venv/Scripts/python.exe -m pytest python/tests -q` | 15 passed; 30 exporter/deprecation warnings |
| Generated OpenAPI/TypeScript contract | In `web`: `npm run contract:check` | Passed |
| Frontend units | In `web`: `npm test` | 4 passed |
| Production frontend | In `web`: `npm run build` | Passed |
| Desktop/mobile browser suite | In `web`: `npm run test:e2e` | 22 passed |
| Final chart layout recheck | In `web`: `npm run test:e2e -- --grep 'captures chart timing'` | 2 passed after mobile date-spacing adjustment |
| Release benchmark smoke run | `cargo bench --manifest-path rust/Cargo.toml --bench feature_benchmarks --bench simulation_benchmarks -- --test` | Both benchmark targets compiled and all workloads passed |

The Python suite includes multi-horizon output/loss behavior, conformal
calibration, chronological splitting, scaler boundaries and PyTorch/ONNX
numerical parity. ONNX exporter warnings remain visible; a passing Python
parity check alone does not certify every possible tract input shape.

## Integration and visual coverage

The Rust CLI integration test ingests explicit synthetic data, validates and
persists it, builds real features, trains a small model, predicts, backtests,
simulates and reads a report. It checks rejection of missing data and reused
test splits, and verifies published provenance. Other Rust checks cover API
contracts, Arrow series, accounting, indicator properties and a deterministic
2,520-bar backtest fixture.

Playwright runs against the Rust server and recorded synthetic fixtures on
desktop Chromium and a Pixel 7 viewport. Coverage includes all seven routes,
backend metrics, chart tables, the unverified-simulation toggle, empty/error
states and screenshots. Visual inspection found clipped equity labels and
crowded mobile date ticks; the chart now reserves more axis space.

Fixtures are synthetic test evidence, not measurements of trading performance.
Tests attach navigation-to-visible-canvas timing, which is a smoke measurement
and does not establish a production chart latency SLA. Browser screenshots and
raw logs remain ignored local outputs.

## Reproduction and limits

Use `scripts/verify.ps1` for the main Windows checks after installing the
documented dependencies and Playwright Chromium. Run Rust executable builds
and the browser server sequentially on Windows to avoid executable file locks.
This session required execution outside the restricted sandbox for Python
launches and esbuild filesystem access; the final results use those reruns.

No live Yahoo download, remote deployment, live orders, GPU acceleration,
production-size load test or numeric performance regression threshold was
verified. Criterion smoke execution checks workload viability, not a latency
target. See [capabilities](14-CAPABILITIES.md) and the
[improvement roadmap](15-IMPROVEMENTS.md) for integration and statistical limits.
