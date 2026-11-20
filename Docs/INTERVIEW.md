# Interview pack

This pack describes the implementation, retained tests, completed development study and measured runtime comparison. It makes no alpha or production-readiness claim. Read [CLAIMS](CLAIMS.md), [the development findings](STUDY-REPORT.md), and [measured benchmarks](BENCHMARKS.md) before presenting it. Any numerical performance claim must come directly from those reports.

## Two-minute pitch

I built a stock forecasting research platform with a Rust data and inference runtime, PyTorch training, and a React inspector. The engineering problem was making it difficult to accidentally present a demo, a leaked experiment, or an incompatible model as a meaningful forecast.

The pipeline versions market snapshots, computes causal features, exports training data through Arrow, and fits scaling only on training folds. Model publication uses staged artifacts, hashes, and numerical parity checks. The API labels synthetic provenance and the UI displays it. Alpaca minute ingestion can supply requested forecasts when the completed-bar window and model are compatible; that path has local tests, while real account acceptance remains human-gated.

I added mutation-tested leakage canaries, optional model signatures, an append-only decision log, and empirical drift monitoring. I then ran the approved, frozen development study against simpler baselines. The results were negative after costs: the mean LSTM Sharpe was better than ridge's but worse than GBM's. I report that result without tuning until the LSTM wins. I also measured the runtimes on the same sealed real-data model; ONNX Runtime was faster in that session, so I do not claim a tract latency advantage. The final holdout remains blocked at the user's request, and the paper session remains human-operated.

## Ten-minute walkthrough

Start with `configs/default.yaml` and the frozen `configs/study_daily.yaml`. Explain why a symbol, date range, source and dataset version are part of the experiment, and why a present-day universe is not survivorship-free.

Follow `rust/cli/src/handlers/data.rs` into versioned market storage, then `handlers/features.rs` into causal features and Arrow training export. Show that an availability timestamp is separate from a bar's nominal timestamp. Historical provider snapshots cannot retroactively certify first-publication history.

Open `python/ml/train_orchestrator.py` and `dataset.py`. Walk through index validation, purging, embargoing, train-only scaling, sequence construction and validation-based stopping. Show the canary tests and the saved mutation evidence; explain what the controls detect and what they cannot establish.

Follow staged model publication into `rust/inference/src/integrity.rs`. Separate numerical parity, hash integrity and optional signature authenticity. Show where the trusted public key enters through configuration and why accepting a key from the artifact itself would defeat the trust boundary.

Follow `rust/api/src/forecast.rs` through source/model compatibility checks and explicit provenance. Show the Alpaca window rules and synthetic banner. Then inspect `decisions.rs` and the `monitor` CLI: hashes link decisions to packages and inputs, later outcomes are appended, and drift can produce a nonzero halt status for a supervisor.

End with the claims ledger and evidence. Show the frozen study runner's approval and one-use holdout guards. Explain the remaining real-data, provider and host gates before discussing further research.

## Architecture

```mermaid
flowchart LR
    Historical[CSV or Yahoo helper] --> Ingest[Rust validation and versioned snapshots]
    Alpaca[Alpaca WebSocket] --> Journal[Capture journal and bounded minute window]
    Journal --> Ingest
    Ingest --> Features[Causal Rust features]
    Features --> Arrow[Arrow training dataset]
    Arrow --> Train[PyTorch training and frozen baseline study]
    Train --> Package[Staged checkpoint and ONNX package]
    Package --> Gate[Hashes, parity and optional trusted signature]
    Gate --> Runtime[tract inference]
    Features --> Runtime
    Journal --> Forecast[Requested compatible live forecast]
    Runtime --> Forecast
    Forecast --> Log[Append-only decisions and outcomes]
    Log --> Monitor[Rolling metrics and feature drift]
    Forecast --> UI[React inspector with provenance]
```

The Node-hosted bridge and standalone Rust application have different public routes and live capabilities. The diagram describes data flow, not a claim that every deployment exposes every path.

## Questions and grounded answers

**Why an LSTM rather than GBM or a transformer?** It is a sequence-model hypothesis and the existing model architecture, not a proven winner. The frozen study compares it with zero, momentum, ridge and gradient-boosted trees using the same feature sequences and evaluation dates. A transformer adds complexity without current comparative evidence. If the final evidence does not establish LSTM superiority, report that plainly and prefer the simpler model where appropriate.

**How did you prevent lookahead and survivorship bias?** Availability checks, forward-label alignment, purged/embargoed splits and train-only scaling address specific leakage paths. Mutation tests demonstrate that the controls are exercised. Survivorship bias has not been eliminated: the frozen universe was selected with present-day knowledge. Provider revisions and corporate actions remain limitations, so do not claim a historically point-in-time universe.

**Why Rust runtime and Python training?** Rust owns data validation, feature computation, inference, APIs and replay, while Python provides model training and export. Python is not exclusively used for training: Yahoo ingestion and study orchestration also use Python helpers. Artifact schemas and parity gates bridge the boundary. The measured real-model comparison is in the benchmark report; it does not establish a universal language or runtime advantage.

**What does the backtest assume about fills, slippage and impact?** The frozen study generates signals after completed closes, enters at the next observed open and exits at that bar's close using fixed per-side costs. It is long-or-cash with independent equal-capital symbol sleeves. These are simulated prices and costs; neither capacity, queue position, dividends, borrow nor market impact is certified. The model's close-to-close target differs from its executable open-to-close return, and both are retained.

**How do you detect decay?** Saved train-only empirical features provide the KS reference. Monitoring selects the exact package and feature schema, excludes synthetic forecasts, deduplicates repeated decisions, and reports drift plus rolling IC and hit rate when outcomes are available. Thresholds are operational choices. A halt exits the monitoring command; the operator must connect that signal to a supervisor. Human-supplied outcomes and causal attribution of drift remain unverified.

**How did you account for multiple testing?** Freeze the universe, dates, costs, models, seeds and caps before fitting. Append every model attempt, including failures, to a durable ledger. Keep final evaluation behind a separate one-use sign-off. Report block-bootstrap uncertainty and an assumption-dependent deflated Sharpe diagnostic using logged attempts. Correlated trials make the effective independent count uncertain. CSCV/PBO is not implemented and must remain unavailable rather than invented.

**Why tract rather than ort?** The implemented Rust runtime loads ONNX through `tract-onnx`; the Python export checks also use ONNX Runtime. This matches the repository's current deployment and operator support. The measured comparison used the same sealed model and fixed windows and passed parity. ONNX Runtime was faster in this session; tract is an architectural choice, not a demonstrated latency advantage. See the benchmark report for the exact host, settings and limitations.

**What is verified?** Local tests cover provenance, artifact rejection, canary mutations, deterministic live-window behavior, monitoring drift and signature rejection. GitHub evidence is revision-specific. Real predictive advantage, actual-provider operation, host shutdown/SSE behavior under deployment conditions, load, retention and restore remain unverified. Consult the claims ledger rather than extrapolating from a passing unit test.

## Resume wording currently supported

- Built a Rust and PyTorch forecasting research platform with versioned data, causal features, train-only preprocessing and parity-gated model publication.
- Added mutation-tested leakage controls, explicit synthetic provenance, optional artifact signatures, and auditable forecast decisions with drift monitoring.
- Built a leakage-safe research platform and evaluated an LSTM against statistical and tree baselines on real-data development folds; reported negative after-cost results without claiming alpha.
- Benchmarked PyTorch, ONNX Runtime and tract on a sealed real-data model, retaining raw timing samples, parity evidence and hardware metadata.

The final holdout remains unevaluated. Attach only numbers reproduced in the study or benchmark reports, and identify development results as development results. No claim of LSTM superiority is supported by this work.

## What I would do with more time

Preserve the blocked holdout until explicitly authorized and collect the human paper-session evidence. Then address historically point-in-time universe construction, corporate actions and data licensing, calibrated monitoring thresholds, independently anchored audit logs, key rotation, and host-specific load/recovery acceptance. Any new experiment needs a new prospective protocol; do not retune the completed study to reverse its findings.
