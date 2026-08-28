# 03 — Model & Training (Python — ML Only)

## Scope Boundary

Python code in this system does exactly four things: define the model, train it,
validate it during training, export it. It never fetches data, computes features,
or runs a backtest. Its sole input is an Arrow IPC dataset produced by Rust; its
sole output is a versioned model artifact directory.

## Model (`ml/models/lstm.py`)

Unchanged core design from the prior baseline — kept because it's appropriately
simple and CPU-friendly, not because it's beyond scrutiny:

```
Input (B, T, F)
  → LSTM(hidden=128, layers=2, dropout=0.3)
  → last hidden state (B, hidden)
  → LayerNorm
  → Linear(hidden→64) → ReLU → Dropout
  → Linear(64→1)
  → predicted log-return (B, 1)
```

Before adding complexity (attention, residual connections, multi-task heads),
each candidate change must clear an experiment: does it improve out-of-sample
IC/ICIR on the *test* fold, not just validation loss? Track this in the
experiment log (below) rather than merging speculatively.

## Walk-Forward Validation (Formalized)

Expanding-window with explicit purge and embargo, not just a gap:

```
[ TRAIN ][ purge ][ VALIDATION ][ embargo ][ TEST ]
```

- **Purge**: drop training samples whose label window overlaps the validation
  window's start (prevents label leakage when targets are multi-day returns).
- **Embargo**: additional buffer after validation before test begins, sized to
  the longest feature lookback, so no feature computed for a test-period
  prediction touches validation-period data.
- Hyperparameters are tuned only against validation folds. The final test fold
  is touched exactly once, at the end, for the reported number that goes in
  any writeup — never used to pick a checkpoint or hyperparameter.

## Target Definition Interface

```python
@dataclass
class TargetDefinition:
    horizon: int              # e.g., 1, 5, 10 (days)
    transformation: str        # "log_return", "vol_adjusted_return", "direction"
    price_field: str = "close"
    target_type: str = "regression"  # or "classification"
```

New targets are added by instantiating this, not by rewriting the pipeline —
the Rust dataset builder tags each row with `target_horizon`, and the Python
trainer simply selects which target column(s) to regress against.

## Versioned Model Artifact

Replaces the bare `best_model.pt`. Written as a directory:

```
models/lstm_v{N}/
├── model.onnx
├── model.pt                  # original PyTorch weights, kept for research reproducibility
├── scaler.json                # fitted normalization params (mean/std per feature)
├── metadata.json
└── training_log.json          # per-epoch metrics, early-stop point
```

`metadata.json` schema:

```json
{
  "model_id": "lstm_v7",
  "model_version": 7,
  "training_dataset_version": "ds_2024_v3",
  "feature_set_version": 4,
  "feature_schema": ["log_return", "rolling_vol_10", "..."],
  "target_definition": {"horizon": 1, "transformation": "log_return"},
  "lookback": 60,
  "architecture": {"hidden_size": 128, "num_layers": 2, "dropout": 0.3},
  "hyperparameters": {"lr": 1e-3, "batch_size": 64, "weight_decay": 1e-4},
  "training_period": ["2015-01-01", "2022-06-30"],
  "validation_period": ["2022-07-15", "2022-10-15"],
  "test_period": ["2022-11-01", "2023-04-30"],
  "random_seed": 42,
  "framework_version": "torch==2.2.0",
  "onnx_opset": 17,
  "evaluation_metrics": {"ic": 0.061, "rank_ic": 0.058, "hit_rate": 0.534},
  "git_commit": "a1b2c3d",
  "created_at": "2026-08-27T10:00:00Z"
}
```

- Rust's inference layer refuses to load an artifact missing any required
  metadata field — this is the enforcement point for "no untracked models in
  production."
- Two training runs with identical `metadata.json` inputs (same seed, dataset
  version, config) must reproduce `evaluation_metrics` within a documented
  numerical tolerance (e.g., IC within ±0.002) — this is a CI-checked property,
  not an aspiration.

## Experiment Tracking

Every training run appends a record (JSON lines file or lightweight SQLite —
no external service required for a solo research platform) containing the same
fields as `metadata.json` plus a free-text hypothesis field. This is what
`RESEARCH_PROTOCOL` in doc 07 audits against.

## Export (`ml/export/onnx_export.py`)

Unchanged responsibility from before, now producing the full artifact directory
above instead of a lone `.onnx` file:

- input name `"input"`, output name `"return_pred"`, dynamic batch axis
- parity check against the PyTorch model (`max abs diff < 1e-4`) runs as part
  of export, not as an optional afterthought — export fails if parity fails
- `scaler.json` written alongside so Rust never needs Python at inference time
