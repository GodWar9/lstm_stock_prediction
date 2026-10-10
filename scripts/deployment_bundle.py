"""Package only explicitly selected, integrity-checked public research artifacts."""
import argparse
import gzip
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import re
import tarfile

MAX_TOTAL = 128 * 1024 * 1024
ID = re.compile(r"[A-Za-z0-9_-]{1,128}")
MODEL_FILES = {"metadata.json", "model.onnx", "model.pt", "scaler.json", "training_log.json", "validation.json"}
RUN_FILES = {"manifest.json", "benchmark.arrow", "equity.arrow", "positions.arrow", "report.json",
             "risk.json", "signal_outcomes.arrow", "signals.arrow", "trades.arrow", "validation.json",
             "simulation.json", "bands.arrow"}

def identifier(value):
    if not ID.fullmatch(value):
        raise ValueError("Invalid artifact ID")
    return value

def read_file(path, limit=64 * 1024 * 1024):
    if path.is_symlink() or not path.is_file() or path.stat().st_size > limit:
        raise ValueError(f"Unsupported artifact: {path.name}")
    return path.read_bytes()

def digest(data):
    return hashlib.sha256(data).hexdigest()

def pack(root, model, dataset, symbol, output):
    for value in (model, dataset, symbol):
        identifier(value)
    files = {}
    def add(relative):
        path = root / relative
        if not path.resolve().is_relative_to(root.resolve()):
            raise ValueError("Artifact escapes root")
        data = read_file(path)
        files[relative] = data
        return data
    model_dir = f"models/{model}"
    seal = json.loads(add(f"{model_dir}/integrity.json"))
    if seal["model_id"] != model or seal["algorithm"] != "sha256" or seal["schema_version"] != 1:
        raise ValueError("Invalid model seal")
    if set(seal["files"]) != MODEL_FILES:
        raise ValueError("Model seal has missing or unexpected files")
    for name in sorted(MODEL_FILES):
        data = add(f"{model_dir}/{name}")
        if seal["files"][name] != {"sha256": digest(data), "size_bytes": len(data)}:
            raise ValueError(f"Model integrity mismatch: {name}")
    metadata = json.loads(files[f"{model_dir}/metadata.json"])
    if metadata["model_id"] != model:
        raise ValueError("Model ID mismatch")
    market = f"datasets/market/{dataset}/{symbol}"
    manifest = json.loads(add(f"{market}.manifest.json"))
    data = add(f"{market}.json")
    if (manifest["dataset_version"] != dataset or manifest["symbol"] != symbol
            or manifest["content_sha256"] != digest(data)):
        raise ValueError("Dataset integrity mismatch")
    runs = []
    for path in sorted((root / "reports/runs").glob("*/manifest.json")):
        run = json.loads(read_file(path))
        if run["provenance"]["model_artifact_id"] != model or run["provenance"]["data_version"] != dataset:
            continue
        run_id = identifier(run["run_id"])
        if run_id != path.parent.name or run["schema_version"] != 1:
            raise ValueError("Run identity mismatch")
        for name in sorted({"manifest.json", *run["artifacts"].values()}):
            if name not in RUN_FILES:
                raise ValueError("Unsupported run artifact")
            add(f"reports/runs/{run_id}/{name}")
        runs.append(run_id)
    if sum(map(len, files.values())) > MAX_TOTAL:
        raise ValueError("Bundle exceeds 128 MiB")
    info = {"schema_version": 1, "model": model, "dataset": dataset, "symbol": symbol,
            "source": manifest["source"], "runs": runs,
            "files": {name: {"sha256": digest(data), "size_bytes": len(data)} for name, data in sorted(files.items())}}
    files["bundle.json"] = (json.dumps(info, indent=2) + "\n").encode()
    output.parent.mkdir(parents=True, exist_ok=True)
    # Stable gzip/tar metadata makes repacking identical inputs reproducible.
    with output.open("wb") as raw, gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as zipped:
        with tarfile.open(fileobj=zipped, mode="w") as archive:
            for name, data in sorted(files.items()):
                entry = tarfile.TarInfo(name)
                entry.size, entry.mode, entry.mtime = len(data), 0o644, 0
                archive.addfile(entry, io.BytesIO(data))
    info["bundle_sha256"] = digest(output.read_bytes())
    output.with_suffix(".json").write_text(json.dumps(info, indent=2) + "\n", encoding="utf-8")
    print(f"Packaged {len(files)} files, {output.stat().st_size} bytes; source={manifest['source']}")

def extract(bundle, output):
    expected = json.loads(bundle.with_suffix(".json").read_text(encoding="utf-8"))
    if digest(read_file(bundle, MAX_TOTAL)) != expected["bundle_sha256"]:
        raise ValueError("Bundle checksum mismatch")
    if output.exists() and any(output.iterdir()):
        raise ValueError("Extraction target must be empty; never overwrite a research workspace")
    files, total = {}, 0
    with tarfile.open(bundle, "r:gz") as archive:
        for member in archive:
            path = PurePosixPath(member.name)
            if (not member.isfile() or path.is_absolute() or ".." in path.parts
                    or "\\" in member.name or ":" in member.name or member.name in files):
                raise ValueError("Unsafe or duplicate bundle member")
            if member.name not in expected["files"] and member.name != "bundle.json":
                raise ValueError("Unexpected bundle member")
            total += member.size
            if member.size < 0 or total > MAX_TOTAL or len(files) > 1000:
                raise ValueError("Bundle exceeds extraction limits")
            data = archive.extractfile(member).read()
            if member.name != "bundle.json" and expected["files"][member.name] != {
                "sha256": digest(data), "size_bytes": len(data)
            }:
                raise ValueError("Artifact checksum mismatch")
            files[member.name] = data
    if set(files) != {*expected["files"], "bundle.json"}:
        raise ValueError("Incomplete bundle")
    inner = json.loads(files["bundle.json"])
    if inner != {k: v for k, v in expected.items() if k != "bundle_sha256"}:
        raise ValueError("Bundle manifest mismatch")
    for name, data in files.items():
        destination = output / name
        if not destination.resolve().is_relative_to(output.resolve()):
            raise ValueError("Artifact escapes extraction directory")
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(data)
    print(f"Verified and extracted {len(files)} files to {output}")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    p = sub.add_parser("pack")
    p.add_argument("--root", type=Path, default=Path("."))
    for name in ("model", "dataset", "symbol"):
        p.add_argument("--" + name, required=True)
    p.add_argument("--output", type=Path, default=Path("deploy/artifacts.tar.gz"))
    p = sub.add_parser("extract")
    p.add_argument("--bundle", type=Path, default=Path("deploy/artifacts.tar.gz"))
    p.add_argument("--output", type=Path, required=True)
    args = vars(parser.parse_args())
    command = args.pop("command")
    (pack if command == "pack" else extract)(**args)
