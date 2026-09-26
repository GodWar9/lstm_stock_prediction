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

Series requests accept `max_points=32..100000` (default 2000). Bucket extrema
selection preserves endpoints and value-column extrema. Downsampled trades are
an inspection sample, not the full audit log; request sufficient points or use
the saved report for a complete audit. Signals require `asof` in UTC milliseconds
and optionally `instrument`; filtering happens before downsampling in Rust.

## HTTP

`quantctl serve --root . --port 8787` binds exclusively to `127.0.0.1`. All
endpoints are GET-only. JSON errors have `code` and `message`.

- `/api/runs`, `/api/runs/{id}/manifest`
- `/api/runs/{id}/equity.arrow`, `trades.arrow`, `signals.arrow`
- `/api/runs/{id}/simulation/bands.arrow`, `simulation.json`
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
embargo segments, scaler isolation, and measured PyTorch/ONNX Runtime parity.
This is one holdout fold, not a claim of orchestrated rolling retraining.
The backend refuses split overlap, empty evaluation windows, mismatched dataset
versions, and unavailable feature schemas. A local model-hash evaluation ledger
requires explicit `--allow-reuse` to evaluate the same test split again.

Simulation uses seeded stationary bootstrap with expected block length five.
Pointwise NAV percentile bands are not forecast confidence intervals. Memory is
bounded at five million path steps. Benchmark, cost sensitivity, position
history, realized signal targets and regime artifacts are not yet persisted;
the inspector must not invent them.
