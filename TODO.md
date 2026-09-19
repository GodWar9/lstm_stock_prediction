# Platform Optimization & Strategy Improvisations Roadmap (TODO)

This document provides the definitive, prioritized engineering and quantitative roadmap for the `lstm_stock_prediction` platform. It focuses on pure software performance engineering (achieving maximum throughput without GPU/hardware dependencies) and mathematical/strategic alpha improvisations.

---

## 1. Pure Software Performance (CPU Optimization without Hardware Acceleration)

### 1.1 Zero-Allocation Hot Paths
- [ ] **Thread-Local Scratchpad Allocators**: Replace per-step vector allocations in `CompositeExecutionModel` and `VolatilityTargetedConstructor` with a thread-local re-usable scratch arena.
- [ ] **SIMD Auto-Vectorization for Indicators**:
  - Implement explicit AVX2/SSE4.1 chunks in `quant_features` for rolling moving averages, exponential smoothing, and true range calculations.
  - Structure contiguous arrays so LLVM emits `vfmadd213pd` (FMA) instructions across contiguous memory slices.
- [ ] **Cache-Aligned Ring Buffers**:
  - Pad `BarWindow` circular buffers to 64-byte L1 cache-line boundaries (`#[repr(align(64))]`) to prevent false sharing and maximize L1 data cache throughput.
- [ ] **Zero-Copy Arrow IPC Streaming**:
  - Implement `mmap`-backed Arrow IPC record batch streaming for `quant_features::export` to load multi-gigabyte datasets without allocating heap memory.

### 1.2 Inference & Runtime Optimization
- [x] **Reciprocal Standard Deviation Precomputation**: Precompute `inv_std = 1.0 / std` in `FittedScaler` to replace slow division cycles (`fdiv`) with single-cycle multiplication (`fmul`).
- [x] **Direct `f32` Single-Pass Scaling**: Normalize and cast `f64` feature slices directly into `f32` tract-compatible buffers, eliminating intermediate vector allocations.
- [x] **Online Welford Accumulation in Monte Carlo**: Replaced vector return collections with $O(1)$ memory online running variance in `MonteCarloResampler`.
- [ ] **Tract Subgraph Constant Folding**: Ahead-of-time graph freezing and static INT8 dynamic quantization for pure CPU tract-onnx execution.

---

## 2. Quantitative Alpha & Signal Strategy Improvisations

### 2.1 Advanced Feature Engineering
- [ ] **Garman-Klass & Parkinson Volatility Estimators**:
  - Incorporate High-Low price extremes into volatility estimations rather than relying solely on Close-to-Close log returns:
    $$\sigma_{GK}^2 = 0.5 \ln\left(\frac{H}{L}\right)^2 - (2\ln 2 - 1) \ln\left(\frac{C}{O}\right)^2$$
- [ ] **Cross-Sectional Z-Score Normalization**:
  - Rank features cross-sectionally across universe members at time $t$ to eliminate broad market drift and capture idiosyncratic alpha.
- [ ] **Fractionally Differentiated Stationarity (FracDiff)**:
  - Implement fractional differentiation of price series ($d \in [0.3, 0.7]$) to preserve memory while achieving stationarity (de Prado).

### 2.2 Model Architecture & Loss Functions
- [ ] **Multi-Horizon Return Forecasting**:
  - Extend the LSTM output head to predict joint 1-day, 5-day, and 20-day cumulative returns with shared temporal representations.
- [ ] **Asymmetric Downside Loss (Sortino Loss)**:
  - Penalize negative prediction errors more severely than positive errors:
    $$\mathcal{L}(y, \hat{y}) = \frac{1}{N} \sum_i \left( (y_i - \hat{y}_i)^2 \cdot (1 + \alpha \cdot \mathbb{I}_{y_i < 0}) \right)$$
- [ ] **Conformal Prediction Uncertainty Intervals**:
  - Generate distribution-free calibrated prediction bands to dynamically modulate signal confidence.

---

## 3. Portfolio, Risk & Execution Simulation

### 3.1 Advanced Portfolio Construction
- [ ] **Bayesian Shrinkage Kelly Criterion**:
  - Implement dynamic position sizing combining expected return $\mu$, volatility $\sigma$, and fractional shrinkage parameter $\lambda \in (0, 0.5]$:
    $$w_i^* = \lambda \cdot \frac{\mu_i - r_f}{\sigma_i^2}$$
- [ ] **Convex Optimization with Sector Neutrality**:
  - Integrate a pure-Rust quadratic programming solver (e.g., OSQP or Clarabel) to enforce sector and factor neutrality constraints.
- [ ] **Regime-Switching Risk Guardrails**:
  - Transition portfolio bounds between Bull, Bear, and Crisis states detected via `quant_simulation::RegimeClassifier`.

### 3.2 Realistic Market Microstructure Simulation
- [ ] **Almgren-Chriss Nonlinear Market Impact**:
  - Replace linear impact with temporary and permanent square-root impact:
    $$\Delta P_{perm} = \gamma \cdot \sigma \cdot \left(\frac{V_{order}}{V_{adv}}\right)^\alpha, \quad \Delta P_{temp} = \eta \cdot \sigma \cdot \left(\frac{V_{order}}{V_{bar}}\right)^\beta$$
- [ ] **Variable Fee Schedules**: Support tiered exchange fees, maker/taker rebates, and borrowing fees for short equity legs.

---

## 4. Benchmarking & Quality Assurance

- [ ] **Criterion.rs Micro-Benchmarks**:
  - Track nanosecond throughput of `FeatureGraph::compute_batch`, `OnnxSession::predict_f32_slice`, and `MonteCarloResampler::generate_paths`.
- [ ] **Automated Performance Regression Gates in CI**:
  - Reject pull requests if inference or feature throughput degrades by more than 5%.
- [ ] **Deterministic Long-Period Golden Backtests**:
  - Verify exact byte-identical replay PnL reports across 10-year historical datasets.
