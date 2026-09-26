> Current behavior: see the [capability guide](Docs/14-CAPABILITIES.md) and [verification report](Docs/16-VERIFICATION.md). Historical checkboxes and latency targets below do not establish CLI integration or measured SLA compliance.

# Platform Optimization & Strategy Improvisations Roadmap (TODO)

This document provides the definitive, prioritized engineering and quantitative roadmap for the `lstm_stock_prediction` platform. It focuses on pure software performance engineering (achieving maximum throughput without GPU/hardware dependencies) and mathematical/strategic alpha improvisations.

---

## 1. Pure Software Performance (CPU Optimization without Hardware Acceleration)

### 1.1 Zero-Allocation Hot Paths
- [x] **Thread-Local Scratchpad Allocators**: Replace per-step vector allocations in `CompositeExecutionModel` and `VolatilityTargetedConstructor` with a thread-local re-usable scratch arena (`quant_execution::scratch`).
- [x] **SIMD Auto-Vectorization for Indicators**:
  - Implement explicit AVX2/SSE4.1 chunks in `quant_features` for rolling moving averages, exponential smoothing, and true range calculations (`quant_features::simd_ops`).
  - Structure contiguous arrays so LLVM emits `vfmadd213pd` (FMA) instructions across contiguous memory slices.
- [x] **Cache-Aligned Ring Buffers**:
  - Pad `BarWindow` circular buffers to 64-byte L1 cache-line boundaries (`#[repr(align(64))]`) to prevent false sharing and maximize L1 data cache throughput (`quant_features::window`).
- [x] **Zero-Copy Arrow IPC Streaming**:
  - Implement `mmap`-backed Arrow IPC record batch streaming for `quant_features::export` to load multi-gigabyte datasets without allocating heap memory (`quant_features::mmap_reader`).

### 1.2 Inference & Runtime Optimization
- [x] **Reciprocal Standard Deviation Precomputation**: Precompute `inv_std = 1.0 / std` in `FittedScaler` to replace slow division cycles (`fdiv`) with single-cycle multiplication (`fmul`).
- [x] **Direct `f32` Single-Pass Scaling**: Normalize and cast `f64` feature slices directly into `f32` tract-compatible buffers, eliminating intermediate vector allocations.
- [x] **Online Welford Accumulation in Monte Carlo**: Replaced vector return collections with $O(1)$ memory online running variance in `MonteCarloResampler`.
- [x] **Tract Subgraph Constant Folding**: Ahead-of-time graph freezing and static INT8 dynamic quantization for pure CPU tract-onnx execution (`OnnxSession::load_optimized`).

---

## 2. Quantitative Alpha & Signal Strategy Improvisations

### 2.1 Advanced Feature Engineering
- [x] **Garman-Klass & Parkinson Volatility Estimators**:
  - Incorporate High-Low price extremes into volatility estimations rather than relying solely on Close-to-Close log returns:
    $$\sigma_{GK}^2 = 0.5 \ln\left(\frac{H}{L}\right)^2 - (2\ln 2 - 1) \ln\left(\frac{C}{O}\right)^2$$
- [x] **Cross-Sectional Z-Score Normalization**:
  - Rank features cross-sectionally across universe members at time $t$ to eliminate broad market drift and capture idiosyncratic alpha (`quant_features::cross_sectional`).
- [x] **Fractionally Differentiated Stationarity (FracDiff)**:
  - Implement fractional differentiation of price series ($d \in [0.3, 0.7]$) to preserve memory while achieving stationarity (de Prado) (`quant_features::fracdiff`).

### 2.2 Model Architecture & Loss Functions
- [x] **Multi-Horizon Return Forecasting**:
  - Extend the LSTM output head to predict joint 1-day, 5-day, and 20-day cumulative returns with shared temporal representations (`MultiHorizonLSTMForecaster`, `MultiHorizonLoss`).
- [x] **Asymmetric Downside Loss (Sortino Loss)**:
  - Maximize returns while penalizing downside semi-deviation below target threshold (`SortinoAwareLoss`).
- [x] **Conformal Prediction Uncertainty Intervals**:
  - Generate distribution-free calibrated prediction bands to dynamically modulate signal confidence (`ConformalPredictor` in `python/ml/conformal.py`).

---

## 3. Portfolio, Risk & Execution Simulation

### 3.1 Advanced Portfolio Construction
- [x] **Bayesian Shrinkage Kelly Criterion**:
  - Implement dynamic position sizing combining expected return $\mu$, volatility $\sigma$, and fractional shrinkage parameter $\lambda \in (0, 0.5]$ (`KellyCriterionConstructor`).
- [x] **Convex Optimization with Sector Neutrality**:
  - Pure-Rust Projected Gradient Descent QP solver with Duchi $L_1$ ball projection, position bounds, and sector neutrality constraints (`quant_portfolio::qp_optimizer`, `quant_portfolio::sector`).
- [x] **Regime-Switching Risk Guardrails**:
  - Transition portfolio bounds between Bull, Bear, and Crisis states detected via `quant_simulation::RegimeClassifier` (`RegimeSwitchingGuardrails`).

### 3.2 Realistic Market Microstructure Simulation
- [x] **Almgren-Chriss Nonlinear Market Impact**:
  - Replace linear impact with temporary and permanent square-root impact:
    $$\Delta P_{perm} = \gamma \cdot \sigma \cdot \left(\frac{V_{order}}{V_{adv}}\right)^\alpha, \quad \Delta P_{temp} = \eta \cdot \sigma \cdot \left(\frac{V_{order}}{V_{bar}}\right)^\beta$$
    (`AlmgrenChrissExecutionModel` in `quant_execution::impact`).
- [x] **Variable Fee Schedules**: Support tiered exchange fees, maker/taker rebates, and borrowing fees for short equity legs (`VariableFeeSchedule` in `quant_execution::fees`).

---

## 4. Benchmarking & Quality Assurance

- [x] **Criterion.rs Micro-Benchmarks**:
  - Track nanosecond throughput of `FeatureGraph::compute_batch`, `FracDiff::apply_slice`, `normalize_cross_section`, and `MonteCarloResampler::generate_paths`.

- [x] **Automated Performance Regression Gates in CI**:
  - Added `bench-gate` job in `.github/workflows/ci.yml` compiling and verifying feature and simulation benchmarks across every pull request.
- [x] **Deterministic Long-Period Golden Backtests**:
  - Implemented 10-year (2,520 trading days) deterministic backtest regression test with committed golden fixture (`rust/backtest/tests/golden_backtest.rs`).
