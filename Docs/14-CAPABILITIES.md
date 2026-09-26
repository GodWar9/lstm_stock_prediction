# Project capability guide

This guide describes the implemented research platform, not a promise of
profitable trading or an exchange-connected execution system. Read
`15-IMPROVEMENTS.md` for the remaining research and engineering work.

## End-to-end workflow

```text
Configured provider -> validated OHLCV JSON -> causal Rust features
    -> Parquet feature table + Arrow training dataset
    -> Python LSTM training and holdout evaluation
    -> ONNX weights + fitted scaler + metadata + validation record
    -> Rust tract inference -> calibrated signals -> next-bar replay
    -> portfolio accounting + simulated fills -> immutable run artifacts
    -> stationary bootstrap scenarios -> localhost API -> React inspector
```

Python owns training and export. Rust owns runtime data processing, inference,
signals, portfolio accounting, execution simulation and HTTP serving. The
existing Yahoo adapter delegates acquisition to a Python helper; this legacy
exception to the desired language boundary is not hidden. Node is needed to
build and test the SPA, but not to serve the compiled application.

## Configuration and CLI

The YAML schema covers provider, symbols, dates, dataset version, feature
lookback and target horizon, model architecture and optimizer settings,
inference settings, exposure limits, transaction costs and initial capital.
Configuration is validated before operational commands. Supported environment
overrides are defined in `rust/config/src/loader.rs`; they are not a general
override for every field. Python reads the YAML training configuration directly.

| Command | Capability / output |
|---|---|
| `config validate`, `config show` | Validate or inspect resolved configuration |
| `data ingest [--symbol AAPL]` | Persist explicitly selected provider output |
| `data validate [--dataset-version ID]` | Reload and validate saved bars |
| `features build [--feature-set baseline_v1]` | Build from saved bars; missing data fails |
| `train [--dataset PATH --manifest PATH]` | Train on Rust Arrow data; optional explicit synthetic smoke mode |
| `predict --model ID --symbol AAPL` | Ordered, schema-checked historical features -> native inference |
| `backtest run --model ID --split test` | Replay the model's recorded split; publish run |
| `simulate --report PATH --paths 1000` | Publish bootstrap summaries and pointwise NAV bands |
| `report --backtest PATH` | Print the existing report |
| `serve --root . --port 8787` | Serve embedded frontend and read-only localhost API |
| `openapi` | Print generated HTTP contract |
| `env` | Build/environment diagnostic information |

Operate from the repository root so datasets, model artifacts and reports have
one unambiguous location. Choose a new model ID for every training run. Test
split reuse requires an explicit `--allow-reuse`; its use is recorded as a run
warning. This ledger protects local evaluations, not independently copied
workspaces or manual deletion of the ledger.

## Market data and point-in-time validation

- Yahoo acquisition adapter and deterministic synthetic provider. Unknown
  providers fail; no implicit synthetic replacement is permitted.
- OHLC values must be finite and positive, candles must be internally
  consistent, and timestamps must be strictly increasing.
- Persistence validates bars and manifest row count; reloading validates bars
  again. Dataset manifests identify symbol, provider, version and date range.
- Separate point-in-time and train/test overlap validators are available in
  the data library. Corporate-action adjustment utilities and US equity
  calendar abstractions are available, but the default pipeline does not
  promise a complete survivorship-free institutional dataset.
- Data artifacts are local version-named files. Dataset version names alone
  are not content hashes or a guarantee against later file changes.

## Features and datasets

The operational baseline graph uses SMA(20), EMA(12), RSI(14), MACD,
Bollinger bands, ATR(14), rolling volatility(20), and one-bar log return.
The old graph inserted two SMAs under the same key; the current graph retains
its effective SMA(20) schema without silently replacing a named feature.

Rows are emitted only after feature warmup. Forward targets are joined by
timestamp and trailing unlabelled rows are excluded. Feature rows are stored
as Parquet; training uses Arrow IPC with a schema manifest. Inference orders
columns by model metadata and rejects missing/nonfinite features.

Additional library components include Parkinson/Garman–Klass volatility,
cross-sectional normalization, winsorization/ranks, fractional differentiation,
cache-aligned windows, contiguous batch arithmetic and a memory-mapped IPC
reader. They are tested building blocks, not automatically selected by the
baseline graph. Mapping the file does not guarantee zero-copy decoded arrays;
the present IPC reader still allocates record-batch buffers.

## Training and model packages

- PyTorch LSTM with a single return head; train-fitted feature scaling,
  directional asymmetric loss, AdamW, checkpoint selection and early stopping.
- Chronological train/validation/test holdout with purge at least as large as
  the target horizon, embargo gaps, explicit label-boundary checks, and enough
  rows required to create evaluation windows. No fabricated zero metrics for
  an empty held-out set.
- Reproducible NumPy/PyTorch seeds, saved training losses, schema, model
  parameters, dataset version, split periods, git revision and export time.
- ONNX export and measured parity against PyTorch on an evaluation window.
  The Python suite additionally tests several batch/sequence shapes.
- `validation.json` records the actual split timeline and parity error. The
  orchestrator performs **one chronological holdout**, not repeated rolling
  walk-forward training. Walk-forward utilities exist separately.
- Multi-horizon LSTM, multi-horizon loss, Sortino-aware loss, Sharpe-aware loss,
  conformal calibration and quantization are research modules. They are not
  selected automatically by the standard training CLI or single-head Rust
  provider. Quantized ONNX compatibility needs a runtime parity gate before use.

Training reserves its model ID before processing data, writes under
`models/.staging`, and publishes the complete directory with a same-filesystem
rename only after export, metadata and finite ONNX parity checks succeed.
Ordinary failures clean up the stage and release the ID; competing training
attempts and existing model directories are rejected. IDs use 1-128 letters,
digits, underscores or hyphens, begin with a letter or digit, and exclude Windows
device names. A killed process or machine failure can leave a hidden stage and
lock: confirm no training process owns that ID before removing its leftovers.
This provides atomic visibility, not power-loss durability or protection against
external manual edits. Legacy checked-in metadata may reference absent weights.

## Inference and signals

The runtime uses tract-onnx with a fixed batch-one input shape, typed graph
optimization, and scaler application. A provider trait separates model choice
from downstream portfolio code. The CLI computes the same baseline features
used by training and fails on schema incompatibility. Legacy synthetic
`feature_0` artifacts must be retrained on the Arrow dataset.

Signal libraries provide direction/deadband calibration, confidence transforms,
threshold filtering and cross-sectional ranking. Backtests record expected
forward return and signal confidence. These scores are not probabilities of
profit. The CLI prediction command prints results; the backtest produces the
persisted signal series for the inspector.

## Portfolio, risk and execution

The operational replay uses volatility-targeted sizing with a fallback asset
volatility estimate, position clipping, gross/net exposure scaling and drawdown
de-risking. It reads supported limits and execution costs from configuration.
Cash, holdings, market prices, commissions and NAV are updated per bar.

Execution simulates signed fills with spread, volume-related slippage,
commissions and participation limits. Signals observable at a previous bar
drive execution at the next bar's modeled close; this avoids filling with the
same close used to calculate the signal. It is a bar simulator, not an order
book or exchange simulator.

Library-only extensions include shrinkage Kelly sizing, sector-neutral projected
optimization, tiered fees/rebates/borrow costs, nonlinear impact and regime
guardrails. VaR/CVaR, stress calculations and derivative abstractions also exist
as libraries. These are not all wired into the default backtest configuration.
Options/futures abstractions do not constitute a complete derivatives strategy.

## Reports and simulation

Reports contain account equity, bar returns, total return, CAGR, Sharpe,
Sortino, drawdown, turnover, fill logs and average-cost realized trade outcomes.
Entry and exit fees are allocated on partial closes and reversals. Open
positions are not labelled winning trades. Turnover is absolute traded notional
over initial capital. The hit-rate denominator is closing fill events, not all
fills or fully matched round trips.

Annualization assumes 252 bars/year. Deflated Sharpe remains a heuristic penalty;
profit factor uses the legacy finite sentinel when there are no losses. Do not
treat either as a rigorous significance statistic. The inspector emphasizes
the supported metrics and exposes raw reports for audit.

Simulation uses a seeded stationary bootstrap, expected block length five,
online variance for summary paths and pointwise NAV percentile bands. The CLI
bounds path count and total path steps. Summary distributions include Sharpe,
drawdown, CAGR and frequency of drawdown >=50%. Simulation Sharpe is computed
without a risk-free subtraction, unlike the configured backtest Sharpe.
Regime classifiers and perturbation interfaces exist; the perturbation adapter
is a seeded resampling baseline, not a full cost/parameter sweep.

## Inspector and API

See `13-ARTIFACT-API.md` for exact schemas and routes. The React/Vite/TypeScript
SPA uses TanStack Router/Query, generated API types, Arrow decoding, a worker
for larger buffers and uPlot canvas charts. A common canvas renderer is used
for equity and dense series rather than maintaining separate chart libraries.

- Overview: active run, source, split, available metrics and equity.
- Backtest: NAV, drawdown, sortable virtualized fills and accessible tables.
- Data and validation: recorded dataset, PIT/scaler checks, fold timeline,
  purge/embargo explanations and exact JSON evidence.
- Models: artifact availability, metadata, losses, measured export parity,
  two-version comparison; no invented per-fold statistics.
- Signals: expected returns, confidence, observations, explicit UTC as-of filter.
- Risk and simulation: pointwise p5/median/p95 NAV curves and outcome summaries.
- Run activity: published artifacts discovered through SSE; no job launcher or
  fake progress percentage.

Every page has a copyable provenance footer. Out-of-sample test runs are the
default; in-sample/unverified inspection requires an explicit toggle. Synthetic
source labels and warnings remain visible. Missing capabilities have actionable
empty states. Charts have tabular alternatives; layouts support mobile and
keyboard navigation with reduced motion respected.

Not yet exported: benchmark equity, per-bar positions, realized signal targets,
calibration/rolling IC, cost sensitivity, regime breakdown and derivative Greeks.
Their absence is visible instead of being fabricated in the browser.

## Verification

Tests cover mathematical units, property checks, leakage/scaling boundaries,
ONNX parity, persistence, realized accounting, long-period deterministic replay,
HTTP routes/schema/errors, Arrow contracts and the full CLI pipeline. Frontend
tests cover Arrow decoding/formatting and desktop/mobile browser routes. CI
checks formatting, clippy, Rust/Python tests, benchmark execution, frontend
build/tests and generated-contract drift. See `16-VERIFICATION.md` for measured
results from this implementation session.
