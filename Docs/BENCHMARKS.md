# Three-runtime inference benchmark

Status: measured on the preselected first AAPL development model after development training completed. No model was selected by performance. These are local CPU measurements, not a deployment SLA or evidence of trading alpha.

Model: `study_436e8abf43eb_AAPL_development_0_17`. Commit: `5cea24d6b9e6c34770a984f1ca02baaf2b482605`.

OS: `Windows-11-10.0.26300-SP0`. Processor identifier: `Intel64 Family 6 Model 183 Stepping 1, GenuineIntel`. Logical CPUs: 32.

[CPU model and physical cores](evidence/phase6/benchmark-hardware-20261118T124412383082Z.txt).

Measured calls per runtime: 10000. Excluded warmup calls per runtime: 100.

CPU batch one, pre-scaled fixed windows, Python runtimes one thread, tract release; includes language-call overhead, excludes loading and scaling.

| Runtime | p50 (microseconds) | p95 (microseconds) | p99 (microseconds) | Calls per second |
|---|---|---|---|---|
| pytorch | 820.8 | 1118.1099999999997 | 1487.6610000000012 | 1159.9008586340092 |
| onnxruntime | 366.9 | 500.20999999999987 | 767.6120000000003 | 2572.288046358804 |
| tract-onnx | 1420.55 | 1950.3299999999997 | 2596.2280000000005 | 762.373065170539 |

## Parity and versions

Maximum absolute prediction differences against PyTorch on the fixed windows:

```json
{
  "onnxruntime": 1.30385160446167e-07,
  "tract-onnx": 2.682209014892578e-07
}
```

Runtime versions:

```json
{
  "torch": "2.14.0+cpu",
  "onnxruntime": "1.30.0",
  "tract-onnx": "0.21.18"
}
```

ONNX Runtime had lower latency in this measurement. The results do not justify claiming tract is faster. This is a single model, host and sequential measurement session; hardware, workload and scheduling affect timings.

## Evidence and reproduction

[Exact command and stdout](evidence/phase6/real-model-runtime-benchmark-20261120T035101046488Z.txt); [raw timing samples and metadata](evidence/phase6/runtime-comparison.json). Tables are generated directly from that JSON.

```powershell
python/.venv/Scripts/python.exe scripts/benchmark_runtimes.py --artifact models/study_436e8abf43eb_AAPL_development_0_17 --runs 10000 --output reports/benchmarks/NEW-runtime-comparison.json
```

Use a new output path. Preserve the sealed local model and fixed windows; generated model packages are not automatically pushed. Loading, scaling and compilation are excluded; language-call overhead is included. Python runtimes use one thread; tract uses the release executable. The real-data source and integrity guards reject synthetic, unknown or changed packages.

The initial preparation command used an incorrect hyphenated Cargo package name and failed before compilation; the corrected `quant_inference` release build succeeded. Both logs are retained under `evidence/phase6/`.

ONNX SHA256: `717843c8db2d35f733ea2d85969daa1d19f3bf22a073e3372c90675a72763544`. Input SHA256: `1c73af4c188836889e274d26e4d43ad45bfa56f065872212ee710cd9adc986dd`.
