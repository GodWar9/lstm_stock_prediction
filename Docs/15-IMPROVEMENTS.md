# Improvement roadmap: software first, hardware second

This roadmap separates changes that improve research validity from optimizations
that make the same calculation faster. Performance targets in BUILD_SPEC are
aspirations until measured on a named machine and workload.

## Without additional hardware

| Priority | Improvement | Why / acceptance evidence |
|---|---|---|
| Done | Bind market/training bytes to model provenance | SHA-256 market/training manifests, replay identity checks and sealed model packages |
| P0 | Expand accounting/property tests for partial fills, shorts, splits, dividends and borrow fees | Compare NAV, cash and realized P&L to hand-calculated ledgers |
| Done | Walk-forward orchestration with a model per fold | Per-fold packages, separated windows, train-only scaling and pooled held-out prediction metrics |
| Done | Rust-runtime parity gate for every exported model | PyTorch/ONNX Runtime/tract checks on held-out windows before publication |
| Done | Atomic model staging and directory rename | Training reserves the ID, validates the staged package and parity evidence, then publishes it; failure, collision and real CLI integration tests cover the lifecycle |
| P1 | Replace finite profit-factor sentinel and heuristic deflated Sharpe | Represent undefined ratios explicitly; implement and validate statistical definitions |
| Done | Persist benchmark, positions, realized signal targets and risk reports | Backend evidence and inspector panels implemented; cost sensitivity/rolling IC/regime remain research extensions |
| P1 | Proper risk integration and constrained optimizer certification | Enforce sector, turnover and regime limits in the actual replay; test constraints after rounding and partial fills |
| P1 | Native Rust market acquisition and reliable corporate-action provenance | Remove the legacy Python acquisition exception; test adjusted histories and calendars |
| P1 | Model selection baselines and uncertainty calibration | Compare naive, linear and LSTM models using identical folds and costs; assess conformal coverage under dependence |
| Done | Bounded API artifact workers and size limits | Four blocking workers, bounded file/list sizes, explicit overload errors; production p95/RSS measurement remains optional profiling |
| Done | Full-blotter pagination | Complete stable pages with counts and desktop/mobile controls; multi-instrument schemas remain outside the single-asset product |
| Done | Working-source fingerprint and dirty-tree provenance | Model metadata and replay provenance include source SHA-256, file count and dirty status |

### Software performance work

1. Establish reproducible release baselines for feature graphs, scaling,
   inference, replay and bootstrap. Record compiler, model shape, input size,
   CPU, thread count, median/p95 and peak memory. Existing Criterion execution
   gates check that benchmarks run; they are **not** numeric regression gates.
2. Replace per-bar HashMap feature rows with a versioned dense column schema and
   reusable buffers. Keep schema ordering checks at the boundary. Benchmark
   allocations as well as time; scratch helper existence is not proof that
   production paths use it.
3. Make incremental indicators update in O(1) where mathematically equivalent.
   Preserve golden indicator and causality tests; do not change an indicator
   definition under the same feature version for speed.
4. Reuse model input tensors and flattening buffers; batch offline predictions
   where the selected backend supports it. Keep batch-one behavior for latency
   measurements and compare p95, not only throughput.
5. Memory-map Arrow safely and use a genuinely zero-copy reader only when the
   format/alignment permits. The current mmap wrapper still decodes via a reader
   that can allocate. Prevent writers from changing mapped files.
6. Parallelize independent symbols/folds/paths with deterministic per-job seeds
   and bounded concurrency. Verify identical results independent of scheduling.
7. Approximate percentile sketches can bound simulation memory, but must expose
   approximation error. Exact pointwise bands currently require O(paths × bars)
   storage and are capped.
8. Cache immutable Arrow responses and generate level-of-detail extrema tiles
   once per artifact. Decode larger payloads in workers; retain accessible
   tables and server-side filtering. Measure time to first chart with real runs.
9. Profile trained model shape before adding hand-written SIMD. Existing batch
   loops permit compiler vectorization; inspect generated assembly and measure
   fallback correctness rather than claiming universal AVX acceleration.

## Optimize for the hardware already available

| Resource | Adaptation | Verification |
|---|---|---|
| CPU cache / memory bandwidth | Contiguous arrays, cache-friendly chunks, fixed feature order, avoid nested thread pools | Cache-miss and allocation profiles on the same dataset |
| CPU instruction set | Portable default build; optional `target-cpu=native` for a local release | Scalar/native parity and deployment CPU compatibility |
| CPU cores | Tune one bounded worker pool; reserve capacity for API/UI | Sweep 1/2/4/... threads, report throughput and tail latency |
| RAM | Memory budgets per dataset, bounded simulation and Arrow batches | Peak working set; fail early instead of swapping |
| Storage | Keep immutable artifacts local; sequential Arrow access; avoid repeated scans | Cold/warm cache timings and artifact integrity checks |
| GPU, if present | Use it for sufficiently large training batches after measuring transfer cost | End-to-end epoch time, validation parity and reproducible seeds |

Do not assume a GPU improves small LSTM inference. The current production
runtime is CPU tract; CUDA/TensorRT would require a new provider, packaging,
shape validation and numerical parity gates. Multi-GPU training and distributed
workers are separate capabilities, not configuration switches that already work.

### Optional hardware expansion, only after profiling

More RAM helps if the measured workload is paging; more cores help independent
folds/symbols until bandwidth saturates; a faster SSD helps cold artifact reads;
a GPU helps large training workloads that amortize transfer and launch overhead.
Choose based on measured bottlenecks rather than spending on an assumed one.

## Optional next delivery sequence

The current product now includes dataset hashes, model/runtime parity, per-fold
walk-forward packages, portfolio/risk/signal evidence, complete audit pages,
bounded API artifact work and working-source fingerprints. Remaining rows above
are broader accounting/research/integration or performance extensions, not hidden
stubs in those delivered workflows.

1. Certify additional corporate-action, short-borrow and constraint scenarios
   before exposing those library extensions as supported replay modes.
2. Profile a representative large dataset and optimize the measured bottleneck.
3. Add pooled trading replay, multi-asset research or model-selection experiments
   only with explicit scope and acceptance criteria.
4. Consider hardware-specific providers after measuring the portable baseline.
