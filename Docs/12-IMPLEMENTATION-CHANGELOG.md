# 12 - Implementation Change Log

This document records the implementation work completed during the platform
migration. It is intended to provide a reproducible engineering history for
future development, code review, and resume documentation.

## Project Direction

The project is being developed as a quantitative research platform rather than
only an LSTM stock-prediction script:

```text
market data
  -> point-in-time validation
  -> Rust feature engine
  -> Arrow training dataset
  -> Python model training
  -> versioned ONNX artifact
  -> Rust inference
  -> signals
  -> portfolio and risk
  -> execution simulation
  -> deterministic backtesting
  -> simulation and reporting
```

The architecture and language boundary are described in
[01-ARCHITECTURE.md](./01-ARCHITECTURE.md). The staged migration is tracked in
[10-MIGRATION-PLAN.md](./10-MIGRATION-PLAN.md), and the active implementation
backlog is in [11-IMPLEMENTATION-PLAN.md](./11-IMPLEMENTATION-PLAN.md).

## Completed Changes

### Legacy boundary cleanup

- Isolated the old Python prototype under `python/research/legacy_v1/`.
- Kept the legacy code as historical reference instead of allowing production
  modules to import it.
- Established the intended boundary where Rust owns data, features, inference,
  signals, portfolio, risk, execution, backtesting, and simulation.
- Python remains responsible for model definition, training, validation during
  training, and artifact export.

### Versioned market-data persistence

Implemented in:

- `rust/data/src/storage.rs`
- `rust/data/src/lib.rs`
- `rust/data/Cargo.toml`
- `rust/cli/src/handlers/data.rs`

Changes:

- Added `DatasetManifest`.
- Added deterministic dataset paths:

  ```text
  datasets/market/<dataset-version>/<symbol>.json
  datasets/market/<dataset-version>/<symbol>.manifest.json
  ```

- Added dataset write and read helpers.
- Persisted dataset version, symbol, date range, row count, and provider source.
- Validated bars before persistence.
- Revalidated bars after reload.
- Rejected empty datasets instead of creating success-shaped output.

### Market-data validation

The ingestion path now validates:

- strictly increasing timestamps
- duplicate timestamps
- finite OHLC values
- positive prices
- `high >= max(open, close)`
- `low <= min(open, close)`

The existing point-in-time checks also cover:

- feature availability versus target observation time
- train/validation/test index overlap
- randomized property-test cases for duplicate and non-monotonic timestamps

### Removed silent fallback behavior

The CLI no longer silently replaces a failed configured provider with synthetic
data. Provider errors now propagate to the caller. Synthetic data remains
available through an explicitly configured synthetic provider for development
and deterministic tests.

This is important for research integrity: a failed production data request must
not appear to be a successful market-data ingestion run.

### Versioned feature output

Implemented in:

- `rust/features/src/export.rs`
- `rust/features/Cargo.toml`
- `rust/cli/src/handlers/features.rs`

Changes:

- Added `FeatureDatasetManifest`.
- Added versioned Parquet output:

  ```text
  datasets/features/<feature-set>/v<version>/<symbol>.parquet
  datasets/features/<feature-set>/v<version>/<symbol>.manifest.json
  ```

- Recorded source dataset version.
- Recorded feature-set version.
- Recorded row count.
- Recorded deterministic sorted feature columns.
- Added a manifest-writing round-trip test.

### Arrow IPC training dataset

Implemented in `rust/features/src/export.rs` and wired through the features
CLI command.

Changes:

- Added `TrainingDatasetManifest`.
- Joined computed feature rows with forward target rows by timestamp.
- Excluded feature rows without a valid forward target.
- Exported typed Arrow IPC columns:

  - `timestamp`
  - `target_timestamp`
  - `target`
  - `asset_id`
  - `feature_set_version`
  - `target_horizon`
  - versioned feature columns

- Added training dataset output:

  ```text
  datasets/training/<dataset-version>/<symbol>.arrow
  datasets/training/<dataset-version>/<symbol>.manifest.json
  ```

- Added a test proving that only rows with matching targets are exported.

### Python Arrow contract

Implemented in:

- `python/ml/arrow_dataset.py`
- `python/tests/test_arrow_dataset.py`
- `requirements.txt`

Added `ArrowTrainingDataset` and `load_arrow_training_dataset`.

The Python loader validates:

- required columns
- non-empty input
- null-free required fields
- strictly increasing feature timestamps
- target timestamps after feature timestamps
- one feature-set version per dataset
- one target horizon per dataset
- presence of feature columns
- manifest row-count consistency
- manifest feature-column consistency

The loader returns typed NumPy arrays for model training and records the
feature schema, target horizon, and feature-set version.

## Verification Completed

### Rust

The following verification passed:

```text
quant_data unit tests: 7 passed
quant_data property tests: 4 passed
quant_features unit tests: 28 passed
quant_features golden tests: 4 passed
quantctl CLI integration tests: 9 passed
full Rust workspace tests: passed
```

### Python

The Arrow loader was executed successfully with the workspace Python
interpreter against a constructed Arrow IPC file and manifest.

The existing Python leakage tests passed:

```text
3 passed
```

The system Python interpreter did not have all workspace dependencies, so the
Arrow test was also validated directly with the configured workspace
`.venv`, where `pyarrow` is installed.

## Meaningful Commits Added

The implementation work added the following commits:

| Commit | Description |
|---|---|
| `9060c73` | Persist validated versioned market datasets |
| `96bc7fe` | Add dataset storage round-trip coverage |
| `6206c94` | Persist versioned feature manifests |
| `b21bd71` | Record completed implementation slices |
| `100ef9d` | Export versioned Arrow IPC training datasets |
| `696151e` | Build training datasets from feature targets in the CLI |
| `95667c2` | Record the Arrow training milestone |
| `e38382a` | Validate Rust Arrow datasets in Python |
| `5df7685` | Record the Python dataset contract |

These commits are intentionally separated by coherent behavior boundaries:
storage, export, CLI wiring, tests, and documentation.

## Current Repository Status

The repository currently contains additional modified files that were not part
of the changes recorded above. They include portfolio, backtest, inference,
configuration, calendar, execution, signals, risk, simulation, model metadata,
and CLI files.

Those changes were already present in the working tree and were deliberately
left untouched. They must be reviewed separately before being included in a
future commit.

Generated test artifacts were removed after validation. Dataset outputs created
by real CLI runs should remain outside source control unless they are
deliberately selected as small fixtures.

## Remaining Work

The next implementation milestones are:

1. Replace synthetic arrays in `python/ml/train_orchestrator.py` with the
   Arrow loader.
2. Record dataset and feature metadata in model artifacts.
3. Add formal dataset-driven walk-forward training.
4. Use real FeatureStore sequences in `quantctl predict`.
5. Replace manual backtest signals with `ModelSignalStream`.
6. Add deterministic real-data backtest fixtures.
7. Complete Monte Carlo stress and benchmark reporting.
8. Add CI gates for Rust, Python, leakage, parity, and artifact validation.
9. Document limitations, data licensing, and research reproducibility.
10. Add derivatives only after the equity pipeline is fully verified.

The target remains more than 100 meaningful commits, with an expected final
range of approximately 110-130 commits. Commit count should come from
independently understandable implementation, test, benchmark, and
documentation changes rather than artificial history inflation.

