# Current product completion checklist

Scope confirmed by the user: finish the current product and its missing
integrations; research experiments and optional hardware optimization are not
acceptance criteria. Each item requires runnable behavior and relevant tests.

- [x] Atomic model publication and model-package integrity.
- [x] Market/training dataset hashes bound to replay provenance.
- [x] PyTorch/ONNX/tract parity before model publication.
- [x] Working checkpoint export and measurable benchmark CLI commands.
- [x] Backend evidence for benchmark, positions, signal outcomes and risk views.
- [x] Inspector displays those evidence artifacts with honest empty states for legacy runs.
- [x] Rolling walk-forward orchestration with per-fold models and held-out results.
- [x] Full test/build checks and corrected documentation.
- [ ] Push completion commit to origin/main (automatic approval review requires destination authorization).

Not part of current-product acceptance: new alpha research, new architecture or
loss experiments, derivatives, multi-asset portfolio research, GPU providers,
hardware purchases, numeric speed targets without a measured baseline, or live
broker execution.

Walk-forward acceptance covers per-fold training packages and held-out prediction
metrics. Root replay uses the final fold; pooled trading replay remains outside
this implementation. See [verification](16-VERIFICATION.md) for current evidence.
