# Local research artifact contract

The current implementation supersedes sketches in `frontend-build-prompt.md`.
Existing backtests were flat JSON reports; simulation previously printed only
summary statistics. There were no saved signals, validation timelines, or API.
The actual inference runtime is **tract-onnx**, not Microsoft's Rust ort binding.

## Publication and identity

`reports/runs/<UUID>/manifest.json` is published after artifacts are written.
Runs have schema version 1, kind, split, UTC creation time, instruments,
capabilities, artifact paths, metric values, warnings, and provenance. Provenance
records git revision, SHA-256 of resolved configuration, dataset version, model
artifact ID and source. Run files are immutable after publication. Re-running
creates a new UUID. The compatibility `reports/backtest_<model>_<split>.json`
is mutable and is not an immutable API resource.

The API rejects unsupported versions and ID mismatches. Only safe artifact IDs
and whitelisted filenames can be read. Canonical paths must remain inside the
configured root. Model files are mutable legacy resources and are not given
immutable cache headers. New training runs refuse to overwrite existing models.

## Model integrity manifest

New models are staged and atomically published with `integrity.json`. Its
`schema_version` is 1, `algorithm` is `sha256`, and `model_id` must match metadata.
The `files` object contains exactly six keys: `model.onnx`, `model.pt`,
`scaler.json`, `metadata.json`, `training_log.json`, and `validation.json`.
Each value contains `sha256` (64 lowercase hexadecimal characters) and
`size_bytes` (the positive byte count). The manifest itself is not hashed.

The Rust prediction provider checks every file before loading the model. Legacy
packages without this manifest must be retrained; the read-only inspector can
still display historical metadata. The manifest detects byte changes but is not
a signature or an authenticated source identity. Keep model files immutable
during loading and use. Dataset-version identity is still separate from model
file integrity.

## Series schema

Arrow IPC **file** format, non-null Float64 columns. UTC timestamp milliseconds
fit exactly in Float64 for supported market history. Original report timestamps
are nanoseconds. Trade and signal artifacts represent the single instrument
declared in the manifest; multi-instrument export needs a new schema version.

| Artifact | Columns, in order | Units |
|---|---|---|
| equity.arrow | timestamp_ms, nav, drawdown | UTC ms, account currency, fraction |
| trades.arrow | timestamp_ms, fill_price, quantity, commission, slippage | UTC ms, currency/share, signed shares, currency, currency/share |
| signals.arrow | timestamp_ms, expected_return, confidence | UTC ms, forward log return, fraction |
| bands.arrow | step, p5, p50, p95 | bar index, account currency |
| benchmark.arrow | timestamp_ms, nav, drawdown | UTC ms, account currency, fraction |
| positions.arrow | timestamp_ms, quantity, market_value | UTC ms, shares, account currency |
| signal_outcomes.arrow | timestamp_ms, prediction_timestamp_ms, expected_return, realized_return, residual | UTC ms, return, return, return |

Chart requests accept `max_points=32..100000` (default 2000). Bucket extrema
selection preserves endpoints and value-column extrema. Audit tables instead use
`offset=0&limit=100` (limit 1..1000): contiguous rows in recorded order, never
sampled. Pagination and `max_points` cannot be combined. Responses include
`X-Total-Count`, `X-Returned-Count`, `X-Offset` and `X-Sampled`. Counts refer to the
filtered dataset. Out-of-range offsets return an empty page. Signals require
`asof` in UTC milliseconds; optional instrument/as-of filters apply before both
sampling and pagination. The inspector uses complete pages for every audit table;
column sorting is explicitly limited to the current page.

## HTTP

`quantctl serve --root . --port 8787` binds exclusively to `127.0.0.1`. All
endpoints are GET-only. JSON errors have `code` and `message`.

- `/api/runs`, `/api/runs/{id}/manifest`
- `/api/runs/{id}/equity.arrow`, `trades.arrow`, `signals.arrow`
- `/api/runs/{id}/benchmark.arrow`, `positions.arrow`, `signal_outcomes.arrow`
- `/api/runs/{id}/simulation/bands.arrow`, `simulation.json`, `risk.json`
- `/api/runs/{id}/validation`, `report.json`
- `/api/models`, `/api/models/{id}`
- `/api/openapi.json`
- `/api/events`: SSE artifact count snapshots every three seconds. This is
  completion discovery, not a job scheduler or invented percentage progress.

Absent capabilities return 404. The frontend gives instructions for producing
them. Unknown API URLs never return the SPA. Immutable run responses carry a
one-year cache lifetime; list queries and model queries must be refreshed.

`quantctl openapi` generates the contract from Rust utoipa definitions. The web
contract script regenerates TypeScript with openapi-typescript and checks drift.

## Validation and limitations

Training records exact chronological train/validation/test windows, purge and
embargo segments, scaler isolation, measured PyTorch/ONNX Runtime parity, and
supports orchestrated rolling walk-forward cross validation with per-fold models.
The backend refuses split overlap, empty evaluation windows, mismatched dataset
versions, and unavailable feature schemas. A local model-hash evaluation ledger
requires explicit `--allow-reuse` to evaluate the same test split again.

Simulation uses seeded stationary bootstrap with expected block length five.
Pointwise NAV percentile bands are not forecast confidence intervals. Memory is
bounded at five million path steps. Benchmark buy-and-hold equity, per-bar
position histories, realized signal outcomes, and parametric risk reports (VaR,
CVaR, beta, stress scenarios) are persisted as backend evidence and inspected with
honest empty states for legacy runs.

Signal outcome `timestamp_ms` is the target realization time; `prediction_timestamp_ms` is the signal time. Returns use the model target horizon and log-return transformation. As-of filtering uses realization time.

Walk-forward packages retain sealed `folds/fold_N` models, validation evidence and held-out predictions. Root inference/replay uses the final fold only; `pooled_oos_metrics` summarizes prediction accuracy across folds, not trading performance.

## Resource bounds and working-source provenance

File reads, Arrow conversion, listing scans and event scans run in blocking workers
behind four shared permits. Requests beyond that budget return `503 busy` without
an unbounded wait queue; SSE skips a busy tick. Cancelling an HTTP request retains
its permit until the underlying work finishes. Individual JSON files are limited
to 8 MiB, Arrow files to 64 MiB, decoded numeric values to four million and columns
to 32. Listings stop at 10,000 entries or 16 MiB of source JSON metadata. Oversized
artifacts return `413`; these are workload guards, not a measured RSS/SLA guarantee.
Symbolic-link artifact files are rejected.

New provenance optionally includes `source_snapshot` with `git_commit`,
`working_tree_dirty`, `content_sha256` and `file_count`. `quantctl source-snapshot`
prints the same structure. The digest hashes a sorted map of repository-relative
source filenames to SHA-256 content hashes (null for tracked deletions), including
untracked non-ignored source. It covers code, build/dependency manifests and config
in rust/python/web/configs/scripts/.github; generated data, browser fixtures and
legacy research are excluded. The dirty flag covers the working repository.
This records files at snapshot time, not a rebuild attestation or a snapshot of
installed dependencies/binary bytes. Run from a Git checkout. Legacy artifacts omit
this field. Training metadata records its own snapshot; backtests record the replay
snapshot, and report-derived simulations retain the source report provenance.
