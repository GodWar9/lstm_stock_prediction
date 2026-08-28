# 10 — Migration Plan

Each stage must compile and pass its tests before the next begins. Nothing is
migrated "big bang."

## Stage 0 — Freeze and Extract What's Reusable
- Freeze the old Python-only prototype (docs 00's inventory) as
  `python/research/legacy_v1/` — read-only reference, not imported by anything.
- Extract into the new system immediately, unchanged in spirit:
  - the config schema shape (pydantic → becomes the Python half of the shared
    config contract, doc 08)
  - the LSTM architecture (`model/lstm.py` → `python/ml/models/lstm.py`)
  - the walk-forward gap concept (formalized with purge/embargo in
    `python/ml/training/walk_forward.py`)

## Stage 1 — Rust Skeleton + Config
- Stand up `rust/` workspace, `quantctl` CLI shell with all subcommands
  registered but most returning `unimplemented!()`.
- Implement `rust/config/` — the shared, validated config loader. Get
  `quantctl config validate` passing against the migrated config schema first;
  everything else depends on this.

## Stage 2 — Data & Feature Engine
- Implement `MarketDataProvider` trait + `YfinanceAdapter`.
- Implement PIT validation layer and leakage checklist (doc 02) — write the
  property tests for it before or alongside the implementation, not after.
- Port the 9 indicators from `features/indicators.py` (pandas) into Rust
  `Feature` implementations. Golden-test each one against the old pandas
  output on a fixed fixture dataset, to prove the port is numerically
  equivalent before the old code is deleted.
- Implement `FeatureStore` (Parquet-backed) and the Arrow IPC dataset writer.

## Stage 3 — Training Path (Python, Reduced Scope)
- Point `python/ml/training/` at the new Arrow IPC dataset produced by Stage 2
  instead of its own pandas pipeline.
- Delete `features/` and `data/` from the Python side entirely once Stage 2's
  golden tests pass — this is the point where "Python touches market data" is
  structurally eliminated, not just discouraged.
- Implement the versioned model artifact writer (doc 03) and the parity test
  (PyTorch ≈ ONNX) as part of `export-model`.

## Stage 4 — Rust Inference
- Implement `OnnxLstmProvider` (doc 04), load a Stage-3-produced artifact.
- Extend the parity test chain to include the Rust `ort::Session` leg.
- `quantctl predict` becomes real.

## Stage 5 — Signal/Portfolio/Risk/Execution
- Implement `Signal`, `PortfolioConstructor`, `RiskEngine`,
  `CompositeExecutionModel` (doc 05) against a stub/synthetic `SignalStream`
  first, so this stage doesn't block on Stage 4's real model output.
- Wire `ModelSignalStream<OnnxLstmProvider>` in once both sides are ready.

## Stage 6 — Backtest & Simulation
- Implement the backtest loop (doc 06) against `SignalStream` — verify it runs
  identically against `ManualSignalStream` (a trivial rule) and
  `ModelSignalStream` (the LSTM), proving the model-agnostic contract holds.
- Implement Monte Carlo/stress simulation.
- Port the old evaluation metrics (IC, Sharpe, etc.) into `RiskEngine`/
  `BacktestReport`, golden-tested against the old Python numbers on the same
  fixture dataset for one final numerical cross-check.

## Stage 7 — Old Code Removal
- Delete `python/research/legacy_v1/`'s active status — keep it only as a
  historical reference, explicitly excluded from CI.
- At this point Python contains only `ml/`; Rust contains everything else;
  the language boundary from doc 01 is fully realized, not aspirational.

## Non-Negotiable Gate Between Every Stage
No stage begins until the previous stage's tests (unit + golden + relevant
property tests) pass in CI. This mirrors doc 07's testing strategy — the
migration itself is held to the same bar as ongoing development, not treated
as a one-time exception.
