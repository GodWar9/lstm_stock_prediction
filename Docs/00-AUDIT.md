# 00 — Audit: Existing System vs. Target Platform

## What Currently Exists

The prior deliverable (ARCHITECTURE.md + agents A–D) was a **single-language Python
research script**, not a platform:

| Component | Language | Status |
|---|---|---|
| Data ingestion | Python (`yfinance`) | Naive — no corporate actions, no PIT correctness, no validation |
| Feature engine | Python (`pandas` + `ta`) | Row-by-row pandas, no versioning, no leakage guards beyond fold-fit |
| Model | Python (`PyTorch`) | Reasonable baseline, correctly scoped |
| Walk-forward CV | Python | Correct concept, but ad hoc — no purge/embargo, no reproducibility record |
| Evaluation | Python (`scipy`) | IC/Sharpe present, no ICIR stability, no stat-sig testing |
| Backtest | Python (`numpy`/`pandas`) | Toy — single-asset, no execution model, no portfolio layer |
| Inference | Python export → ONNX, **Rust listed as stretch goal** | Wrong framing — this is the core boundary, not an afterthought |
| Config | YAML + pydantic | Reasonable foundation, keep |
| Entrypoints | 4 disconnected scripts (`train.py`, `evaluate.py`, `predict.py`, `export.py`) | No CLI, no coherent command surface |

## What Works and Should Be Retained

- **Config schema approach** (pydantic + YAML) — becomes the single Rust+Python-shared config contract.
- **Walk-forward CV concept** (expanding window, gap) — correct instinct, needs purge/embargo formalized and moved fully under reproducibility tracking.
- **Model architecture** (LSTM → LayerNorm → MLP head) — CPU-friendly, appropriately simple; keep as v1 baseline, gate complexity behind experiment evidence.
- **IC/Rank IC/Sharpe/hit-rate metric set** — correct metric family, needs ICIR + stat-sig additions.
- **ONNX export target** — correct interop choice, just needs to be the *primary* interface, not a stretch add-on.

## What Is Weak or Incorrect

1. **Python owns everything.** Feature computation, backtesting, and portfolio logic were all Python/pandas — this is the core violation of the new language boundary and the main source of unbounded technical debt.
2. **No point-in-time (PIT) correctness enforcement.** Normalization was fit per-fold correctly, but there was no systemic guard against feature/label leakage, corporate-action leakage, or universe leakage — it relied on the author remembering to do it right each time.
3. **No model artifact versioning.** `best_model.pt` is a bare checkpoint — no dataset version, feature version, git commit, or environment metadata. Not reproducible.
4. **No instrument abstraction.** Everything assumes "equity, close price, one symbol at a time." Options/futures would require a rewrite, not an extension.
5. **No portfolio/risk/execution separation.** Backtest directly converted `sign(prediction)` into PnL — no position sizing, no transaction-cost realism beyond a flat bps constant, no risk limits.
6. **No experiment tracking / reproducibility contract.** Nothing recorded seeds, splits, or config alongside results.
7. **No CLI.** Four independent scripts with implicit ordering and no validation of prerequisites.
8. **Backtest coupled to the LSTM.** `evaluate.py` imports `model/lstm.py` directly — a different model (XGBoost, linear, ensemble) couldn't reuse the backtester without editing it.
9. **No testing beyond a wishlist** — test files were named but not populated with real fixtures/goldens.
10. **Performance was unaddressed.** Pure pandas/Python throughout means the "Rust for latency" story didn't exist anywhere except the stretch-goal README stub.

## What Should Be Removed

- `backtest/engine.py`, `backtest/strategy.py` (Python) — logic moves to Rust; Python keeps zero backtest code.
- `evaluation/metrics.py`, `evaluation/plots.py` as production paths — retained only as **research-side** notebooks/scripts, not part of the deployable system.
- `features/indicators.py` (pandas-based) — reimplemented in Rust; Python's feature code becomes at most a thin adapter for research parity-checks against the Rust engine.
- The four disconnected `scripts/*.py` — replaced by a single `quantctl` CLI (Rust) that calls the Python trainer as a subprocess/artifact producer.

## Entrypoint Audit

| Entrypoint | Language | Purpose | Problem | Replacement |
|---|---|---|---|---|
| `scripts/train.py` | Python | fetch→features→train→export | Mixes concerns Python shouldn't own (fetch, features) | `quantctl data ingest` + `quantctl features build` (Rust) → `python -m ml.train` (Python, model only) |
| `scripts/evaluate.py` | Python | load ckpt → backtest → plot | Backtest logic in Python, coupled to LSTM import | `quantctl backtest run` (Rust, model-agnostic via `PredictionProvider`) |
| `scripts/predict.py` | Python | fetch→features→ONNX predict | Duplicates feature logic from training path — drift risk | `quantctl predict` (Rust, shares one feature engine binary with training data prep) |
| `inference/export.py` | Python | torch→ONNX | Correct, keep as Python ML-layer responsibility | `python -m ml.export` — unchanged role, now writes a versioned artifact instead of a bare `.onnx` |

## Root Cause

The original design treated this as **"an ML project with a backtest bolted on."**
The corrected design treats it as **"a quant research platform where the LSTM is one
pluggable prediction source."** That inversion is what the rest of these docs implement.

See `01-ARCHITECTURE.md` for the corrected system design.
