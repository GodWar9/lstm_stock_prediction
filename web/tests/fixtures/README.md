# Browser fixtures

These are real outputs of `quantctl` using the explicitly synthetic
`configs/demo.yaml`: ingest, features, training, backtest, then simulation of the
saved immutable backtest report (500 paths). No metrics were authored for UI
display. The source label remains synthetic and the original provenance is kept.

Regenerate with `node scripts/capture-fixtures.mjs` from `web` after running the
pipeline. Model weights are intentionally excluded; the Models page reports
that the fixture registry contains metadata without deployable weights.

Browser tests copy these outputs into a temporary local workspace and start
the actual Rust server. Only empty/error state tests intercept API responses.
