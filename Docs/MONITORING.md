# Decision logging and model monitoring

Successful forecast requests append a durable record to `reports/decisions/events.ndjson` before returning. The response's `decision_id` identifies the hash-chained record. Each record retains model and dataset identity, package hash, feature schema hash, feature-window hash, input-bar hash, provenance, forecast and timestamps. Logging failure rejects the request rather than silently losing its evidence.

Outcome backfill is a new event, not an edit to the forecast:

```powershell
quantctl record-outcome --log reports/decisions/events.ndjson --decision-id HASH --realized-log-return 0.01 --observed-at-ms TIMESTAMP
quantctl monitor --log reports/decisions/events.ndjson --model-dir models/MODEL --window 100 --warn-ks 0.2 --halt-ks 0.4
```

Supply the observed forward log return for the exact forecast horizon after it becomes available. The command rejects unknown decision IDs, duplicate outcomes, nonfinite values and outcomes dated before the input. It does not independently certify a human-supplied outcome or horizon alignment. Retain the data source and calculation separately.

Newly trained packages contain a deterministic empirical reference sampled from training rows only. Monitoring compares the latest feature vectors against that reference using per-feature two-sample KS distance; the maximum determines configurable warn/halt thresholds. It prints rolling Pearson IC and sign hit rate where outcomes exist. A halt returns a nonzero CLI exit; this signals the supervisor and does not stop a separate server process or place orders.

Monitoring requires the exact model package and feature schema, excludes synthetic forecasts, and deduplicates repeated requests for the same dataset/as-of time. Insufficient windows and undefined IC are explicit; no zero metric is substituted. Old models without a reference must be retrained. KS thresholds are operational settings, not calibrated hypothesis-test probabilities; overlapping windows remain dependent.

The log is bounded, process-locked and synced after appending. Archive closed logs deliberately before the capacity limit; no records are automatically deleted. A crash can leave an incomplete tail which is rejected on the next read. The hash chain detects changed records, sequence gaps and partial records, but cannot prove authenticity or detect wholesale rewriting or removal of a complete suffix without an independently retained checkpoint. Keep trusted head hashes and backups outside the workspace when stronger audit guarantees are needed.
