"""One-command measured benchmark; no synthetic investment-performance results."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
import numpy as np
import onnxruntime as ort
import torch
from python.ml.models.lstm import LSTMForecaster


def measure(call, runs):
    for i in range(100):
        call(i)
    samples = []
    for i in range(runs):
        start = time.perf_counter_ns()
        call(i)
        samples.append((time.perf_counter_ns() - start) / 1000)
    return samples


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--artifact', type=Path, required=True)
    parser.add_argument('--runs', type=int, default=10000)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.runs < 10000 or args.output.exists():
        raise ValueError('Require >=10000 runs and a new output path')
    directory = args.artifact.resolve()
    metadata = json.loads((directory / 'metadata.json').read_text())
    source = metadata.get('source', '').lower()
    if not source or source == 'unknown' or any(tag in source for tag in ('synthetic', 'fixture', 'mock')):
        raise ValueError('Reported benchmark requires a real-data model with source provenance')
    integrity = json.loads((directory / 'integrity.json').read_text())
    for name, digest in integrity['files'].items():
        if Path(name).name != name:
            raise ValueError('Invalid package member')
        raw = (directory / name).read_bytes()
        if hashlib.sha256(raw).hexdigest() != digest['sha256'] or len(raw) != digest['size_bytes']:
            raise ValueError('Package integrity failure')
    validation = json.loads((directory / 'validation.json').read_text())
    scaler = json.loads((directory / 'scaler.json').read_text())
    features = len(metadata['feature_schema'])
    raw = np.array([case['raw_features'] for case in validation['runtime_parity']['cases']])
    x = ((raw.reshape(-1, metadata['lookback'], features) - scaler['mean']) / scaler['std']).astype(np.float32)
    torch.set_num_threads(1)
    arch = metadata['architecture']
    model = LSTMForecaster(features, arch['hidden_size'], arch['num_layers'], arch['dropout'])
    model.load_state_dict(torch.load(directory / 'model.pt', map_location='cpu', weights_only=True))
    model.eval()
    tensors = [torch.from_numpy(window[None]) for window in x]
    options = ort.SessionOptions()
    options.intra_op_num_threads = options.inter_op_num_threads = 1
    session = ort.InferenceSession(str(directory / 'model.onnx'), sess_options=options, providers=['CPUExecutionProvider'])
    name = session.get_inputs()[0].name
    with torch.inference_mode():
        pt = lambda i: model(tensors[i % len(x)])
        onnx = lambda i: session.run(None, {name: x[i % len(x)][None]})[0]
        timings = {'pytorch': measure(pt, args.runs), 'onnxruntime': measure(onnx, args.runs)}
        reference = np.array([pt(i).item() for i in range(len(x))])
        predictions = {'onnxruntime': np.array([onnx(i).item() for i in range(len(x))])}
    result = subprocess.run(['cargo', 'run', '--release', '--locked', '--manifest-path', str(ROOT / 'rust/Cargo.toml'),
                             '-p', 'quant_inference', '--example', 'runtime_benchmark', '--', str(directory), str(args.runs)],
                            cwd=ROOT, check=True, capture_output=True, text=True)
    print(result.stderr)
    rust = json.loads(result.stdout)
    timings['tract-onnx'] = rust['samples_us']
    predictions['tract-onnx'] = np.array(rust['predictions'])
    errors = {key: float(np.max(np.abs(values-reference))) for key, values in predictions.items()}
    if any(not np.isfinite(error) or error > 1e-5 for error in errors.values()):
        raise ValueError(f'Runtime parity failed: {errors}')
    report = {'commit': subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
              'os': platform.platform(), 'processor': platform.processor(), 'logical_cpus': os.cpu_count(),
              'model': metadata['model_id'], 'model_sha256': hashlib.sha256((directory/'model.onnx').read_bytes()).hexdigest(),
              'input_sha256': hashlib.sha256(x.tobytes()).hexdigest(), 'runs': args.runs, 'warmup': 100,
              'scope': 'CPU batch one, pre-scaled fixed windows, Python runtimes one thread, tract release; includes language-call overhead, excludes loading and scaling',
              'versions': {'torch': torch.__version__, 'onnxruntime': ort.__version__},
              'max_abs_parity_difference': errors, 'samples_us': timings,
              'summary': {key: dict(zip(['p50_us','p95_us','p99_us'], map(float,np.percentile(values,[50,95,99]))),
                                   throughput_per_second=1e6/float(np.mean(values))) for key,values in timings.items()}}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open('x') as output:
        json.dump(report, output, indent=2, allow_nan=False)
    print(json.dumps({key:value for key,value in report.items() if key != 'samples_us'}, indent=2))


if __name__ == '__main__':
    main()
