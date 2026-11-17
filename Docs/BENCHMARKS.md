# Three runtime inference benchmark

Status: tooling implemented; measurements BLOCKED pending a sealed real-data model and its fixed parity windows. The reported benchmark refuses synthetic or unknown-provenance packages. No latency, throughput or parity number is claimed here.

```powershell
python/.venv/Scripts/python.exe scripts/benchmark_runtimes.py --artifact models/REAL_MODEL --runs 10000 --output reports/benchmarks/runtime-comparison.json
```

The command measures PyTorch, ONNX Runtime and release-mode tract against the same ONNX/checkpoint package and scaled windows. Loading, scaling and warmup are excluded; language-call overhead is included. The JSON retains every timing sample, percentile summaries, throughput, parity differences, model/input hashes, runtime versions, hardware, OS and commit. Python runtimes use one thread. This measures local CPU execution, not market performance or a deployment SLA.

The minimum measured iteration count and warmup are enforced by the tooling. The script refuses an existing output path, a changed package, missing source provenance, or a parity discrepancy above its predefined tolerance. Compile/guard evidence is under `Docs/evidence/phase6/`. A refused demo package is validation of the guard, not a successful benchmark.
