# Quantitative Research Platform: Technical Build Specification (BUILD_SPEC)

## 1. System Architecture & Language Boundary

The platform is designed as an ultra-high-performance quantitative research and execution system. It separates model definition & training (Python) from production data ingestion, feature generation, inference, risk, backtesting, and simulation (Rust).

```text
[ Market Data Ingestion ] (Rust: quant_data)
           │
           ▼ (PIT Validation & Deduplication)
[ Real-Time Feature Engine ] (Rust: quant_features)
           │
           ├───────────────────────────────┐
           ▼                               ▼
[ Arrow IPC Training Dataset ]    [ FeatureStore Memory Ring ]
           │                               │
           ▼ (uv-managed virtualenv)       ▼ (Zero-Copy f32 Slice)
[ Python PyTorch LSTM Training ]  [ Pure-Rust ONNX Inference ] (quant_inference)
           │                               │
           ▼ (INT8 ONNX Export)            ▼
[ Versioned Model Artifact ] ────▶ [ Signal Calibration & Filter ] (quant_signals)
                                           │
                                           ▼
                                  [ Portfolio Constructor ] (quant_portfolio)
                                           │
                                           ▼
                                  [ Risk Guardrails Engine ] (quant_risk)
                                           │
                                           ▼
                                  [ Execution & Impact Model ] (quant_execution)
                                           │
                                           ▼
                                  [ Deterministic Backtester ] (quant_backtest)
                                           │
                                           ▼
                                  [ Monte Carlo & Stress Test ] (quant_simulation)
```

---

## 2. Hardware-Independent Performance Specifications (SLA)

All components must outperform baseline quantitative tooling strictly through software engineering, cache efficiency, and algorithmic design—without requiring GPUs or dedicated accelerators.

| Component | Target Metric | SLA Threshold | Optimization Mechanism |
|---|---|---|---|
| **Feature Graph Computation** | Latency per bar | $\le 450\text{ ns}$ | Contiguous ring-buffer iteration, SIMD multiplication |
| **Feature Scaling (`FittedScaler`)** | Throughput | $\ge 25,000,000\text{ elements/s}$ | Precomputed reciprocal std dev ($1/\sigma$), vector auto-vectorization |
| **Inference (`OnnxSession`)** | Latency per sequence | $\le 45\text{ }\mu\text{s}$ (CPU) | Tract-onnx optimized plan, single-pass `f32` tensor loading |
| **Event Replay Backtest** | Replay speed | $\ge 400,000\text{ bars/s}$ | Zero-allocation point-in-time streaming |
| **Monte Carlo Simulation** | Path throughput | $\ge 60,000\text{ paths/s}$ | Online Welford variance accumulation ($O(1)$ heap allocations) |

---

## 3. Mathematical Specifications

### 3.1 Feature Normalization
For raw feature $x_{t, i}$ and trained scaler parameters $(\mu_i, \sigma_i)$:
$$z_{t, i} = (x_{t, i} - \mu_i) \times \text{inv\_std}_i, \quad \text{where } \text{inv\_std}_i = \frac{1}{\max(\sigma_i, 10^{-12})}$$

### 3.2 Signal Calibration & Deadband Filter
Given raw model predicted forward return $\hat{y}_t$ and confidence score $c_t$:
$$\text{direction} = \begin{cases} \text{Long}, & \text{if } \hat{y}_t > \theta_{\text{long}} \\ \text{Short}, & \text{if } \hat{y}_t < -\theta_{\text{short}} \\ \text{Flat}, & \text{otherwise} \end{cases}$$
The calibrated signal weight scales linearly with confidence above the activation threshold:
$$w_t = \text{sign}(\hat{y}_t) \times \min\left(1.0, \frac{|\hat{y}_t| - \theta}{\theta_{\text{sat}} - \theta}\right) \times c_t$$

### 3.3 Volatility-Targeted Position Sizing with Drawdown De-risking
Given target annualized portfolio volatility $\sigma_{\text{target}}$, current rolling asset volatility $\sigma_{\text{asset}}$, and peak equity $E_{\text{peak}}$:
$$\text{Base Weight } w_{\text{base}} = \frac{\sigma_{\text{target}}}{\sigma_{\text{asset}} \times \sqrt{252}}$$
$$\text{Drawdown } D_t = \frac{E_{\text{peak}} - E_t}{E_{\text{peak}}}$$
$$\text{Multiplier } M(D_t) = \begin{cases} 1.0, & D_t \le D_{\text{warn}} \\ 1.0 - \frac{D_t - D_{\text{warn}}}{D_{\text{max}} - D_{\text{warn}}}, & D_{\text{warn}} < D_t < D_{\text{max}} \\ 0.0, & D_t \ge D_{\text{max}} \end{cases}$$
$$\text{Final Position Weight } w_t^* = w_{\text{base}} \times M(D_t) \times \text{clamp}(w_t, -w_{\text{max}}, w_{\text{max}})$$

### 3.4 Execution Slippage & Linear Impact
Execution fill price $P_{\text{fill}}$ under half-spread $s$ and linear impact factor $\lambda$:
$$P_{\text{fill}} = P_{\text{close}} \times \left(1 + \text{sign}(Q) \cdot \left[ s + \lambda \cdot \frac{|Q|}{V_{\text{bar}}} \right]\right)$$

### 3.5 Online Welford Path Variance
For Monte Carlo sample paths, variance is computed online without storing intermediate return vectors:
$$M_{1, k} = M_{1, k-1} + \frac{x_k - M_{1, k-1}}{k}$$
$$M_{2, k} = M_{2, k-1} + (x_k - M_{1, k-1})(x_k - M_{1, k})$$
$$\sigma_N^2 = \frac{M_{2, N}}{N - 1}$$

---

## 4. Contract Specifications & File Formats

### 4.1 Arrow IPC Feature Dataset
- Path: `datasets/training/<dataset_version>/<symbol>.arrow`
- Format: Arrow IPC Stream File, uncompressed or LZ4
- Mandatory Schema Fields:
  - `timestamp`: Int64 (nanoseconds epoch)
  - `target_timestamp`: Int64
  - `forward_return`: Float32
  - `feature_set_version`: UInt32
  - `target_horizon`: UInt16
  - `feature_0` through `feature_N`: Float32

### 4.2 Model Artifact Bundle
Every deployable model must reside in `models/<model_id>/` and contain:
1. `model.onnx`: INT8 or FP32 ONNX graph exported with opset $\ge 17$. Dynamic or static batch dimension `[1, lookback, num_features]`.
2. `scaler.json`: Immutable JSON with `mean`, `std`, and optional `feature_names`.
3. `metadata.json`: Full provenance containing architecture, train/validation/test date boundaries, git commit SHA, and historical evaluation metrics.

---

## 5. Verification & Testing Matrix

| Layer | Test Type | Tooling | Enforcement |
|---|---|---|---|
| **Syntax & Style** | Lints & Formatting | `cargo fmt`, `cargo clippy` | `RUSTFLAGS="-D warnings"` |
| **Numerical Parity** | Pytorch vs ONNX | `pytest tests/test_onnx_parity.py` | Maximum absolute tolerance: $10^{-5}$ |
| **Leakage Guard** | Temporal Invariants | `pytest tests/test_dataset_leakage.py` | Train/Val/Test zero index overlap |
| **Indicator Goldens** | Precision Verification | `indicator_golden_tests.rs` | Wilder SMA/RSI/Bollinger against reference tables |
| **Platform Determinism**| Identical Replay | `stage5_integration_tests.rs` | Identical inputs produce byte-identical JSON reports |
