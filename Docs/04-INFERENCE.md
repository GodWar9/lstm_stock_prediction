# 04 — Inference (Rust — Core Architecture, Not a Stretch Goal)

## Why This Moved From "Stretch" to "Core"

The prior design put a Python ONNX runner in the main path and mentioned a Rust
`axum`/`ort` server as an optional add-on. That's backwards: if Rust only
appears behind a feature flag, none of the downstream systems (signal engine,
portfolio, backtest — all specified as Rust) can depend on it without either
duplicating a Python inference path or blocking on the stretch goal actually
shipping. Rust ONNX inference is now the **only** inference path in the system.
Python never serves predictions.

## `PredictionProvider` Trait (Model-Agnostic)

```rust
pub trait PredictionProvider {
    fn predict(&self, sequences: &SequenceBatch) -> Result<PredictionBatch>;
    fn model_id(&self) -> &str;
    fn feature_schema(&self) -> &[String];
}

pub struct OnnxLstmProvider {
    session: ort::Session,       // loaded once at startup, reused across calls
    scaler: FittedScaler,        // deserialized from the artifact's scaler.json
    metadata: ModelMetadata,     // deserialized from metadata.json
}
```

- Everything downstream of this trait (signal engine, backtester) depends only
  on `PredictionProvider`. Swapping the LSTM for XGBoost or an ensemble later
  means implementing this trait once — zero changes to signal/portfolio/backtest
  code. This directly satisfies "the production system must not know the model
  is an LSTM."
- `OnnxLstmProvider::load(artifact_dir: &Path) -> Result<Self>` refuses to
  construct if `metadata.json` is missing required fields, if the ONNX opset
  is unsupported, or if `feature_schema` doesn't match what the caller's
  feature engine produces — fail at startup, not mid-inference.

## Session Lifecycle

- One `ort::Session` per loaded model, created once at process startup (or on
  hot-reload of a new model version), never per-request.
- Thread pool sizing (`ort` intra/inter-op threads) is a config value, tuned
  via the benchmark suite in `07-PERFORMANCE-TESTING.md` — not hardcoded.
- Graph optimizations enabled (`GraphOptimizationLevel::All`); FP32 is the
  baseline dtype. FP16/int8 are evaluated only after benchmarking shows the CPU
  target actually benefits — this is a measurement gate, not a default.

## Inference Modes

```rust
fn predict_batch(&self, seqs: &SequenceBatch) -> Result<PredictionBatch>;   // offline/backtest use
fn predict_one(&self, seq: &Sequence) -> Result<Prediction>;                // live single-asset
fn predict_stream(&self, rx: Receiver<Sequence>) -> Receiver<Prediction>;   // future live trading
```

Backtesting always uses `predict_batch` for throughput. A future live-trading
integration uses `predict_one`/`predict_stream` without touching the model
loading or session code.

## Sequence Construction in Rust

- The **same** feature engine (doc `02-DATA-FEATURES.md`) used to build the
  training dataset is used to build inference-time sequences — this was a
  real bug risk in the old design, where `scripts/predict.py` reimplemented
  feature logic in Python separately from the training pipeline and could
  silently drift out of sync. Now there is exactly one feature
  implementation, called from both the dataset-builder CLI command and the
  inference path.
- Rolling feature state (e.g., 60-bar lookback window) is maintained
  incrementally for streaming use — an `O(1)` ring-buffer update per new bar,
  not an `O(lookback)` recompute.

## Prediction → Signal

```rust
pub struct Signal {
    pub direction: Direction,       // Long, Short, Flat
    pub expected_return: f64,
    pub confidence: f64,            // derived from model + historical calibration, not raw softmax
    pub horizon: Duration,
    pub instrument: InstrumentId,
    pub model_id: String,           // traceability — see Observability, doc 08
    pub as_of: Timestamp,
}
```

This is deliberately richer than "predicted float → sign()". It's what makes
the signal layer usable by options/futures strategies later (doc
`09-DERIVATIVES-ROADMAP.md`) without a rewrite.

## Traceability

Every `Prediction` and downstream `Signal` records `model_id`,
`feature_set_version`, `as_of` timestamp, and the input sequence's checksum —
any PnL result in the backtest/live system can be traced back to the exact
model artifact and feature snapshot that produced it (doc
`08-DEPLOYMENT-CONFIG.md` → Observability).

## Parity Testing (Enforced, Not Optional)

`tests/parity/` runs on every artifact build:

```
PyTorch(model.pt) output
        ≈ (tol 1e-4)
ONNX(model.onnx) output
        ≈ (tol 1e-4)
Rust ort::Session(model.onnx) output
```

A model artifact that fails any leg of this chain is rejected by
`quantctl export-model` — it never reaches `models/`.
