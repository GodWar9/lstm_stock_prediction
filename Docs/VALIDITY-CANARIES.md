# Leakage canary protocol

These are synthetic tests and are excluded from strategy performance claims.
They do not touch the proposed real-data study or its final holdout.

- **Future feature:** the Rust PIT validator rejects a feature whose true availability is after the decision time. Availability metadata must be truthful; a timestamp validator cannot discover arbitrary hidden mathematical lookahead.
- **Purge and embargo:** training now validates externally supplied fold indices before any scaler fit. A fold with disjoint labels but an insufficient configured gap must fail, not silently train.
- **Train-only scaler:** the test intercepts the actual orchestrator fit call, introduces a large shift outside the training fold, and verifies that only the training rows reach the scaler. Existing walk-forward integration tests also compare saved scaler statistics against each fold's training rows.
- **Shuffled-label control:** the production LSTM class and trainer fit a fixed synthetic fixture with independently permuted training and held-out labels. The fixed confidence interval and absolute-IC bound are defined in the test before execution. The result is a negative control, not a proof that all leakage is impossible.

Run positive tests with:

```powershell
python/.venv/Scripts/python.exe -m pytest python/tests/test_validity_canaries.py -v -s
cargo test --manifest-path rust/Cargo.toml -p quant_data --locked lookahead_feature_canary
```

The serial mutation command temporarily disables the PIT guard, removes split separation, fits the scaler on all rows, and removes the shuffled-label intervention. It requires assertion failures rather than compiler/import failures, restores the original bytes in `finally`, and prints the original source hashes. Run with no concurrent source edits, tests or builds:

```powershell
python/.venv/Scripts/python.exe scripts/check_canary_mutations.py
```

Removing label shuffling mutates the test control, not a production feature: shuffled labels are intentionally not part of normal training. The other mutations affect production protections. Each mutation was detected in [the retained mutation run](evidence/phase4/mutations-20261116T140816381471Z.txt). Normal sources were restored before regression validation.
