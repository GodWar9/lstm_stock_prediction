# System audit, offline operation and remaining work

Audit date: 2026-09-27. Baseline: commit `a0075f3`, plus the changes described below.
Scope: **offline operation after one connected setup**, as requested. This is a
local research simulator and inspector, not a live trading platform.

## 1. What you can rely on

The intended local workflow is:

```text
Your local CSV -> validated versioned bars -> Rust features and Arrow dataset
  -> Python LSTM training -> sealed ONNX model with Rust parity checks
  -> Rust prediction/backtest -> bootstrap simulation -> localhost inspector
```

The application now defaults to local CSV ingestion. Yahoo acquisition requires
an explicit `QUANTCTL_ALLOW_NETWORK=1`; otherwise both its Rust adapter and Python
helper reject the operation before importing/calling the online provider. There
is no silent download or synthetic-data fallback. Synthetic demo mode remains
explicit and must never be treated as evidence of real-market performance.

The inspector binds to `127.0.0.1`, embeds its frontend, rejects foreign HTTP Host
names, and sends a Content Security Policy restricting browser connections and
scripts to its own origin. ONNX Runtime telemetry is explicitly disabled in the
training/export path. Running a prepared binary needs no Node server.

**This is offline-capable application behavior, not OS-enforced isolation.** No
network adapters or firewall rules were changed. Python extensions, the browser,
the OS, and other installed applications are outside the application's network
policy. Disconnect networking or apply your organization's outbound firewall
policy for an actual isolated machine. This checkout is inside **OneDrive**:
move a prepared copy outside cloud-synced folders or disable syncing if your data
must remain confined to the machine. Do not simply copy a Python virtual
environment to a different computer/path and assume it is portable.

## 2. Verified defects addressed

| Gap found in the code | Change made | Regression evidence |
|---|---|---|
| Default ingestion downloaded from Yahoo; no local import adapter | Default `csv` provider, configurable `data.input_dir`, strict local OHLCV reader | CSV ingestion/validation CLI test and parser tests |
| Online helper could be invoked directly | Both Rust and Python require exact network permission value `1` | Rust denial test; Python tests for missing/false/malformed permission |
| Corrupt CSV rows could disappear, and invalid volume became zero | Local imports reject malformed rows, nonfinite/invalid prices, missing timezone, duplicates and invalid volume; online OHLCV parser also rejects malformed rows/volume | Local parser rejection tests |
| Ingestion could replace an already-used dataset version | Create-new file writes; existing data is never overwritten | Re-ingestion rejection and unchanged-content checks |
| Path-like dataset/symbol identifiers could escape expected locations | Portable identifier checks before storage access and CLI ingestion | Traversal/reserved-name tests |
| NaN/infinite/invalid numeric configuration could pass comparisons | Finite/range checks, actual calendar-date parsing, positive architecture/capital checks | Configuration regression cases |
| Rust config overrides did not reach Python training | Rust writes its resolved, validated config to a temporary YAML file passed to Python | Real training uses overridden model ID and exactly one overridden epoch |
| Training ignored `training.weight_decay` | Configured value reaches AdamW and saved metadata | Full-pipeline test checks nondefault `0.02` |
| GPU training left the model on GPU before CPU evaluation | Move model to CPU before evaluation/export | CPU pipeline passes; CUDA path still needs hardware testing |
| Missing training config became empty defaults | Missing files now fail explicitly | Python missing-config test |
| Multiple configured symbols silently trained/replayed only the first | Training and backtest reject multiple symbols with an actionable message | Code validation; full multi-asset support remains absent |
| Insufficient feature history could print success without usable output | Empty feature warmup now fails; unsupported feature set/target labels are rejected | CLI test with only two bars |
| Verification always ran `npm ci`, requiring registry access | Verification uses installed dependencies and Cargo offline mode by default; installs require an explicit switch | Offline verification run |
| Loopback server accepted foreign Host names and lacked a browser egress policy | Host validation and same-origin CSP, no-referrer and nosniff headers | API tests and browser tests with external requests blocked |
| Documentation example iterated over a Result instead of its batches | Corrected the example | Rust documentation test |

Existing dataset files are not deleted, migrated or repaired by these changes.
For a new ingestion, choose a new dataset version. Keep existing versions and
model packages together with their reports.

## 3. Test evidence

| Check | Result |
|---|---|
| Rust workspace tests, including documentation | 162 passed in the combined run; the added CSV end-to-end test subsequently passed with all 8 CLI tests (163 distinct Rust tests total) |
| Python tests with Python sockets/DNS blocked | 49 passed; 39 existing ONNX tracing/export/deprecation warnings |
| Frontend units and production build | 4 tests passed; TypeScript and Vite build passed |
| Browser tests | 28 passed across desktop and mobile Chromium, including external-request blocking on all seven routes |
| Formatting and workspace/all-target Clippy | Passed; Clippy rerun after the final integration-test addition |
| Generated OpenAPI/TypeScript contract | Passed, no schema drift |
| Benchmark smoke execution | All 8 workloads passed across feature and simulation suites; no throughput SLA claimed |
| Release executable and readiness check | Final build/readiness result recorded below after completion |

Environment: Windows, Rust/Cargo 1.96.0, Node 24.12.0, Python 3.14.4.
CI uses Ubuntu and Node 22, which remain separate validation targets. Test data
is synthetic, including the CSV end-to-end fixture. Both provider paths exercise
actual small-model training, export/runtime parity, prediction, backtest, simulation,
reused-test rejection and model-integrity rejection.

Local raw logs: `audit-verification.log`, `audit-csv-pipeline.log`,
`audit-release-build.log`. Logs and generated test artifacts are git-ignored.

The full command is `./scripts/verify.ps1 -Benchmarks` from PowerShell in the
repository root. It checks formatting, workspace/all-target Clippy, a Rust binary
build, Rust tests, Python tests, frontend unit/build checks, generated API contract,
desktop/mobile Chromium tests, and benchmark smoke execution. Cargo runs offline,
and acquisition permission is disabled. Python tests block Python socket/DNS
calls; the browser offline test aborts any external HTTP request on all seven
inspector routes. These tests do not intercept native-library sockets or establish
a machine-wide no-egress guarantee.

Local results are for Windows, not a claim that GitHub's Ubuntu job passed. The
previous CI linker fix reduces build jobs to two, omits dev/test debug symbols,
disables incremental builds, refreshes caches, separates test compilation and
execution, and reports memory/disk information on failure. The original Linux
signal-7 crash has not been reproduced locally or attributed to a measured resource
exhaustion event. A GitHub Actions run is still required to verify that environment.

## 4. One-time connected preparation

Keep the same prepared OS, architecture, repository location and Python installation
for offline operation. Install Rust stable with rustfmt/Clippy and native build
tools, Python 3.14, uv, Git and Node 22. Windows needs MSVC build tools. Keep the
repository's `.git` directory: training and backtest source provenance require it.

Run these while connected, from the repository root:

```powershell
uv python install 3.14
Push-Location python
uv sync --locked
Pop-Location
Push-Location web
npm ci
npx playwright install chromium
npm run build
Pop-Location
cargo fetch --manifest-path rust/Cargo.toml --locked
cargo build --manifest-path rust/Cargo.toml --bin quantctl --release --locked
```

Then verify the prepared environment without dependency downloads:

```powershell
$env:QUANTCTL_ALLOW_NETWORK = '0'
$env:CARGO_NET_OFFLINE = 'true'
python/.venv/Scripts/python.exe scripts/offline_doctor.py
./scripts/verify.ps1 -Benchmarks
```

Do not run `npm ci`, `uv sync` without offline/cache settings, Playwright install,
or toolchain updates after disconnecting. Use the existing virtual environment
and browser installation. `verify.ps1 -InstallDependencies` explicitly allows
frontend installation and Cargo network access; it does not install Python or
the browser for you. The regular script is Windows-oriented. On Linux use the
commands in README with `python/.venv/bin/python` and `CARGO_NET_OFFLINE=true`.

`offline_doctor.py` is a read-only dependency/source check. It does not prove that
your data is correct, that a model package is valid, or that an old executable
embeds the newest frontend. Rebuild Rust after building/changing the frontend.

## 5. What you must supply or enter

### Real input data

Supply **one CSV per symbol**, for example `datasets/import/AAPL.csv`:

```csv
timestamp,open,high,low,close,volume
2024-01-02T21:00:00Z,100,102,99,101,100000
2024-01-03T21:00:00Z,101,103,100,102,120000
```

Those two rows illustrate the format only; they are not a training dataset or
an independently verified market-data sample. Required rules:

- Exact six-column header shown above; plain comma-separated numeric fields,
  without quoted fields or thousands separators.
- Timestamps must include an explicit timezone and represent when the bar is
  available. Convert exchange sessions, daylight saving and early closes correctly.
  Date-only timestamps are rejected rather than guessing a closing time.
- Bars must be strictly increasing with no duplicates. Prices must be finite and
  positive; high/low must contain open/close. Volume must be a nonnegative integer.
- Date filtering is **UTC `[start_date, end_date)`**, with an exclusive end date.
  The entire file is validated before filtering, including out-of-range rows.
- Supply consistent adjustments for splits/dividends and a documented source.
  The CSV path does not fetch corporate actions or certify adjustment quality.
- Use daily bars for current 252-bar/year statistics. Intraday data needs explicit
  annualization/calendar work first.
- Supply enough history for warmup plus train/validation/test windows and gaps.
  Each split must retain at least `lookback + 1` rows. The usual split is 70/15/15
  before purge/embargo handling; several years of daily history are more useful
  than a minimal sample. Small samples may correctly fail validation.

### Your configuration

Copy `configs/default.yaml` to `configs/offline.yaml` and edit:

| Input | What to enter / decide |
|---|---|
| `data.provider` | `csv` for real offline data; `synthetic` only for demos |
| `data.input_dir` | Local CSV directory, relative to repository root or an absolute local path |
| `data.symbols` | Exactly one symbol for a training/backtest run, matching the filename |
| `data.start_date`, `end_date` | Real history range; end is exclusive |
| `data.dataset_version` | New unique version, e.g. `aapl_daily_20260927` |
| `training.model_id` | New unique model ID, e.g. `aapl_offline_v1`; existing packages cannot be overwritten |
| `inference.model_version` | Set consistently, but predict/backtest select their model via `--model` |
| `features.lookback`, `target_horizon` | History window and forward-return horizon in bars; ensure sufficient data |
| `features.feature_set`, `target_transformation` | Keep `baseline_v1` and `log_return`; other CLI pipelines are not implemented |
| Training parameters | Hidden size, layers, dropout, learning rate, weight decay, epochs, batch size, seed, purge and embargo gaps |
| `backtest.initial_cash`, `risk_free_rate` | Account capital and annual rate assumption |
| Portfolio settings | Intended exposure limits, volatility target and supported long/short mode |
| Execution settings | Cost, spread, slippage and participation assumptions appropriate to your dataset |

Use model IDs containing letters, numbers, underscores and hyphens, starting with
a letter or digit. Symbols may also contain dots, such as `BRK.B`. Do not use path
separators or Windows reserved names. Do not reuse the checked-in `lstm_v1` model
ID: legacy metadata is not a usable, freshly trained model package.

### Run it offline

From the repository root, after editing the config and supplying CSVs:

```powershell
$env:QUANTCTL_ALLOW_NETWORK = '0'
$q = './rust/target/release/quantctl.exe'
& $q --config configs/offline.yaml config validate
& $q --config configs/offline.yaml data ingest
& $q --config configs/offline.yaml data validate
& $q --config configs/offline.yaml features build
& $q --config configs/offline.yaml train
& $q --config configs/offline.yaml predict --model aapl_offline_v1 --symbol AAPL
& $q --config configs/offline.yaml backtest run --model aapl_offline_v1 --split test
& $q serve --root . --port 8787
```

Check each command's exit code before continuing. Open `http://127.0.0.1:8787`.
Replace the model/symbol examples with your chosen values. Preserve the printed
run ID. In another terminal, use its immutable report for simulation:

```powershell
& ./rust/target/release/quantctl.exe simulate --report reports/runs/<run-id>/report.json --paths 500
```

The simulation is explicitly unverified/in-sample in the inspector. Enable the
corresponding toggle to view it. Reusing the test split requires deliberate
`--allow-reuse`; repeated tuning against it weakens held-out evidence.

For a demonstration without real data, use `configs/demo.yaml` with new model and
dataset IDs. Re-running ingestion against its existing version now fails on
purpose; do not delete previous research just to reuse an ID.

## 6. Remaining drawbacks and problems

| Priority | Remaining issue | Consequence / work required |
|---|---|---|
| High | Real-market quality is user supplied | CSV validation catches structural errors, not bad vendor data, survivorship bias, missing sessions or incorrect adjustments. Build a documented data-quality/point-in-time process. |
| High | Single-symbol operational pipeline | No synchronized multi-asset portfolio replay. Build aligned calendars, per-asset signals, multi-asset cash/risk accounting and tests. |
| High | No machine-wide egress enforcement | Use physical disconnection or OS policy for a true air gap; application checks do not control every dependency/process. |
| High | Backups and recovery are manual | Preserve data, models, reports and Git source together. Test restoration. Abrupt termination can leave staging locks or incomplete dataset files; inspect after confirming no process owns them. |
| High | Integrity manifests are unsigned | Hashes detect accidental changes relative to a manifest; someone who can rewrite files and manifests can replace evidence. Add signed manifests and trusted key distribution if authenticity is required. |
| Medium | Dataset publication is not one atomic multi-file transaction | Create-new writes prevent normal replacement; a crash can leave orphaned data before its manifest. Readers fail closed, but recovery is manual. Derived feature/Arrow files are still rebuildable and not fully immutable. |
| Medium | Local filesystem trust is assumed | This is not a multi-user sandbox; writable directories, junctions/symlinks and concurrent edits require OS access controls. |
| Medium | Walk-forward pooled metrics are prediction metrics | The root model/replay uses the final fold. Build pooled chronological trading replay before interpreting pooled metrics as strategy P&L. |
| Medium | Simplified execution and risk | Bar fills, static/default assumptions and fallback volatility are not an order-book simulation. Calibrate costs/slippage/borrow and implement the needed extensions. |
| Medium | Statistical approximations remain | 252-bar annualization; heuristic deflated Sharpe; finite sentinel profit factor with no losses; simulation and backtest Sharpe use different risk-free conventions. Standardize and label them before serious comparison. |
| Medium | Limited model/schema integration | Multi-horizon, calibration, quantization and advanced portfolio/risk libraries are not all connected to the default CLI. Each needs its own parity and replay gates. |
| Medium | Some config knobs are not operational runtime controls | Inference thread/batch settings are not wired into the fixed batch-one tract session. Audit and remove or implement unused controls before relying on them for throughput. |
| Medium | Invalid environment override text can be ignored | Numeric `QUANTCTL_*` overrides with unparsable values still leave YAML values in place. Prefer `config show`; stricter parsing remains future work. |
| Medium | Resource limits and scale remain finite | API JSON limit 8 MiB, artifact limit 64 MiB, four artifact workers; simulation limit five million path steps. CLI/data training largely loads files into memory. Test with your actual volume; no production RSS/latency SLA is established. |
| Medium | Optional Yahoo path is not part of offline acceptance | It needs explicit permission and optional `yfinance`, which is not in the core locked environment. Live fetching, its timestamp conventions and corporate-action parsing were not validated here. Prefer importing approved offline data. |
| Medium | Toolchain/runtime portability | Rust toolchain follows stable; offline dependency caches are OS-specific. The prepared virtual environment and native libraries must remain installed. Fresh installation on a never-connected machine was not built or tested. |
| Medium | CUDA not exercised | CPU evaluation transition is corrected, but GPU drivers, determinism and end-to-end CUDA behavior still need hardware validation. |
| Low | ONNX exporter warnings remain | Existing tracing/deprecation warnings accompany passing parity tests. Plan an exporter migration and rerun all shapes/runtime gates; do not suppress warnings as a substitute. |
| Low | No live order, account or job-management system | There are no broker credentials to enter. Live execution, authentication/remote hosting and a web job launcher would be separate projects. |

Tests reduce known failure modes; they do not establish profitability, universal
correctness, malicious-input safety or production reliability. No live-market
download, external security scan, large-data load test, firewall packet capture,
power-loss recovery test, fresh-machine installation or remote CI pass is claimed.

## 7. What to build next

1. **For your first useful offline experiment:** prepare trustworthy daily CSVs,
   choose the config inputs above, reserve enough held-out history, run the full
   pipeline, and inspect the actual evidence rather than demo metrics.
2. **For repeatable research:** add a signed experiment/config/data catalog,
   restore-tested backups, explicit calendar/adjustment lineage, transaction-safe
   artifact publication, and a policy for test-set reuse.
3. **For strategy evaluation:** implement pooled walk-forward trading replay,
   realistic calibrated costs, consistent statistics and multiple-testing controls.
4. **For broader portfolios:** wire multi-asset replay, cross-sectional features,
   capital allocation and joint constraints with accounting/regression fixtures.
5. **For scale/deployment:** measure memory/latency on your real workload, stream
   large datasets, validate a pinned toolchain and produce reproducible offline
   installation bundles if a never-connected-machine requirement is added later.

## 8. Where to look in the repository

- `rust/data/src/adapters/local_csv.rs`: local parser and provider.
- `rust/data/src/storage.rs`: version writes and data-integrity checks.
- `rust/config/src/validation.rs`: configuration validation.
- `rust/cli/src/handlers/train.rs`: resolved config passed to Python.
- `python/ml/train_orchestrator.py`: training, evaluation and parity.
- `rust/api/src/server.rs`: loopback inspector, host policy and CSP.
- `scripts/verify.ps1`: default-offline development verification.
- `scripts/offline_doctor.py`: dependency/source readiness check.
- `Docs/14-CAPABILITIES.md`: existing capability boundaries.
- `Docs/13-ARTIFACT-API.md`: artifact/API contract and resource limits.

Keep this report with the corresponding source revision and test logs. Re-run
verification after changing dependencies, datasets, models or the runtime platform.
