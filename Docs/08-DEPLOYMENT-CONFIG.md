# 08 — Deployment, Config, CLI & Observability

## Single Entrypoint: `quantctl` (Rust)

```
quantctl data ingest        --config configs/default.yaml
quantctl data validate      --dataset-version ds_2024_v3

quantctl features build     --feature-set baseline_v1

quantctl train               --config configs/train.yaml     # shells out to python -m ml.train
quantctl export-model        --model-version lstm_v7           # shells out to python -m ml.export, then parity-checks

quantctl predict              --model lstm_v7 --symbol AAPL
quantctl backtest run         --model lstm_v7 --split test
quantctl simulate             --report backtest_2024_v3.json --paths 1000
quantctl report               --backtest backtest_2024_v3.json

quantctl benchmark            --suite inference
```

- `train` and `export-model` are the only commands that invoke Python, as a
  subprocess with a well-defined I/O contract (Arrow dataset path in, artifact
  directory path out) — everything else is pure Rust.
- Undocumented ad hoc scripts are not permitted as entrypoints; anything a user
  or CI needs to run is a `quantctl` subcommand.

## Configuration System

Single authoritative source, avoiding the old design's risk of drift between
YAML, Python constants, and CLI args:

```
configs/default.yaml         # base config, versioned in git
  ↓ overridden by
environment variables        # QUANTCTL_* prefix, for deployment-specific values
  ↓ overridden by
CLI flags                    # explicit per-invocation overrides
```

- Config is validated at startup via a schema (Rust `serde` + `validator`,
  mirrored by the Python pydantic schema for the fields Python actually reads)
  — an invalid config fails immediately with a specific error, not partway
  through a multi-hour training run.
- Python's pydantic config and Rust's serde config both deserialize the same
  `config.yaml`; the shared schema fields (feature set version, lookback,
  target definition) are the contract between the two languages, checked for
  consistency by a `quantctl config validate` command.

## Research vs. Production Separation

```
python/research/     # notebooks, exploratory scripts — never imported by python/ml or rust/
python/ml/            # production training path — tested, no notebook dependencies
rust/                 # production runtime, always
```

A notebook can call into `python/ml/` functions for exploration, but nothing
in `python/ml/` or `rust/` ever imports from `python/research/`.

## Observability

Every prediction and every backtest run emits structured logs (JSON lines)
containing at minimum:

```
model_id, feature_set_version, dataset_version, timestamp,
instrument_id, input_checksum, prediction, signal, latency_ms
```

- Metrics (throughput, latency percentiles, error rates) exported in a
  Prometheus-compatible format from the `quantctl` process — no external
  service required to consume them locally, but the format doesn't require
  rework to plug into one later.
- Data-quality alerts: the validation layer (doc 02) emits a distinct log
  level/event for PIT violations, missing bars, and stale data, so these are
  greppable/alertable separately from normal operational logs.

## Failure Handling

| Failure | Behavior |
|---|---|
| Missing/stale market data | Reject the affected bar range; do not silently forward-fill across a gap larger than a configured threshold |
| Malformed data | Validation layer rejects at ingestion, never reaches the feature engine |
| Model artifact unavailable/corrupted | `PredictionProvider::load` fails at startup, process does not start serving |
| Feature schema mismatch (model expects features the current feature set doesn't produce) | Fail fast at model load, not at first prediction |
| Inference failure (mid-batch) | Batch fails atomically with a traceable error; no partial/silently-null predictions enter the signal engine |
| Market closed / invalid timestamp | Calendar module (doc 02) rejects the request explicitly |
| Network failure (data provider) | Retried with backoff up to a configured limit, then surfaced as a data-quality alert, not swallowed |

The general principle carried through every layer: **fail loud and early,
never fail silent and late** — this is a direct response to the old design's
biggest structural risk (silent leakage, silent drift between train/predict
feature logic, silent overwrite of feature-store history).

## Security

- No credentials in source or in `configs/*.yaml` committed to git — provider
  API keys read from environment variables only, with `configs/` git-ignoring
  any `*.local.yaml` override file.
- `development` / `testing` / `production` environments select config
  overrides via `QUANTCTL_ENV`, never via editing the base config file.

## Deployment Shape

A modular monolith (single `quantctl` binary + one Python training
environment), not microservices — there's no operational reason yet to split
inference, backtest, and portfolio into separate deployed services for a
research platform run by one person. The trait boundaries (`PredictionProvider`,
`SignalStream`, `ExecutionModel`) are what make a future split possible without
a rewrite, not an argument for doing it now.
