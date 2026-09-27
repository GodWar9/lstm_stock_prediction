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
