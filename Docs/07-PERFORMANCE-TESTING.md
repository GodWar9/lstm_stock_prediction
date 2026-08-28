# 07 — Performance & Testing

## Performance Philosophy

Benchmark first, optimize the measured hot path second. The likely biggest wins,
in expected order of impact, based on where the old design's actual overhead was:

1. **Eliminating repeated feature recomputation** — Python's pandas pipeline
   recomputed indicators from scratch on every call; the Rust `FeatureStore`
   (doc 02) memoizes and persists by version, so this cost is paid once.
2. **Removing serialization in hot loops** — the old `scripts/predict.py`
   round-tripped through pandas DataFrames and numpy conversions per call; Rust
   keeps data in contiguous Arrow/ndarray buffers end-to-end.
3. **Model loaded once, not per-request** — already fixed structurally in doc 04.
4. **Backtest loop as a tight Rust loop over contiguous arrays** instead of a
   pandas `apply` — expect this to dominate the old backtester's wall-clock
   cost more than any inference latency does.
5. Only after 1–4 are measured and addressed: consider SIMD, Rayon parallelism,
   or FP16 inference — and only where a benchmark shows the current
   implementation is actually the bottleneck.

No latency or throughput number is asserted anywhere in this doc set without a
corresponding `benchmarks/` entry that produced it. "Sub-millisecond" claims
require a criterion benchmark output attached, not an estimate.

## What Gets Benchmarked

| Path | Tool | Metrics |
|---|---|---|
| Data ingestion throughput | criterion | bars/sec |
| Feature computation | criterion | features/sec, cache hit rate |
| Sequence construction | criterion | sequences/sec |
| ONNX inference (batch) | criterion + ort profiling | p50/p95/p99 latency, throughput |
| ONNX inference (single) | criterion | p50/p95/p99 latency |
| Signal → order generation | criterion | orders/sec |
| Backtest full loop | criterion | bars/sec end-to-end |
| Monte Carlo simulation | criterion | paths/sec |
| Memory | valgrind/heaptrack or `dhat` | peak RSS per stage |

Reports are stored under `benchmarks/results/` with the git commit and config
hash, so performance regressions show up in diffs across commits, not just in
a one-time README claim.

## Memory Layout Decisions (Referenced From Doc 02)

- SoA/columnar (Arrow/Parquet) for storage and cross-sectional access.
- AoS/row-major contiguous only at the LSTM sequence-construction boundary,
  where PyTorch/ONNX genuinely wants per-timestep contiguous vectors.
- Ring buffers for rolling-window feature state — `O(1)` update, no
  reallocation per bar.
- Explained per-subsystem in code comments, not just this doc, so the "why"
  travels with the implementation.

## Testing Strategy

### Unit Tests
- Every `Feature` implementation: indicators, rolling stats, returns —
  verified against hand-computed values on small fixed inputs.
- Normalization: fit/transform correctness, boundary enforcement (val/test
  code paths cannot call `fit`).
- Portfolio math: position sizing, constraint enforcement, exposure calculations.
- PnL: fill → position → equity curve arithmetic, verified against
  hand-computed small scenarios.
- Transaction costs: each `ExecutionModel` component in isolation.

### Property Tests (via `proptest` in Rust)
- Zero position → zero PnL contribution, for any market state.
- Zero return bar → no equity curve change, for any position.
- Duplicate timestamp in ingested data → validation failure, always.
- Feature with availability timestamp after target timestamp → dataset build
  failure, always, for any feature/target combination.
- Random signal sequences → Sharpe distribution centered near zero (sanity
  check that the backtest engine isn't structurally biased).

### Integration Tests
Full pipeline run on a small fixed synthetic dataset (checked into
`tests/fixtures/`, not fetched live):
```
ingest → validate → features → dataset → (stub) predict → signal → portfolio
  → execution → backtest → report
```
Assert the report's shape and a few known-value checks (e.g., a
deterministic synthetic price series with a known optimal strategy should
produce a Sharpe within a tolerance band).

### Golden Tests
- Feature outputs, predictions (on a frozen dummy ONNX model), and PnL for a
  fixed input dataset are stored as golden files. Any diff beyond floating-point
  tolerance fails CI and requires an explicit, reviewed golden-file update —
  this catches silent behavior drift (e.g., an indicator formula changing
  without a version bump).

### Parity Tests
Already specified in doc 04 — PyTorch ≈ ONNX ≈ Rust `ort` output, tolerance
1e-4, enforced at artifact-export time, re-verified in CI on every commit that
touches inference code.

### Regression Tests
- Backtest determinism: same inputs → byte-identical `BacktestReport` across
  runs and across machines (guards against nondeterministic float reduction
  order in parallelized code paths — if Rayon is used anywhere in a
  numerically sensitive reduction, this test will catch order-dependent
  drift).

## CI Gate Summary

A change cannot merge if it:
- fails any unit/property/integration/golden/parity test
- fails the leakage checklist (doc 02) on the fixture dataset
- introduces a benchmark regression beyond a configured threshold without an
  explicit justification recorded in the PR
