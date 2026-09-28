"""Read-only offline readiness check. Run with the project's prepared Python environment."""
import importlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def main():
    failures = []
    checks = []
    def check(label, ok, detail):
        checks.append({"check": label, "passed": bool(ok), "detail": detail})
        if not ok:
            failures.append(label)

    check("offline policy", os.environ.get("QUANTCTL_ALLOW_NETWORK") != "1",
          "Unset QUANTCTL_ALLOW_NETWORK or set it to 0 for offline use")
    check("Python version", sys.version_info >= (3, 14), sys.version.split()[0])
    for module in ["numpy", "pandas", "pyarrow", "yaml", "scipy", "torch", "onnx", "onnxruntime"]:
        try:
            imported = importlib.import_module(module)
            if module == "onnxruntime":
                imported.disable_telemetry_events()
            check(module, True, getattr(imported, "__version__", "installed"))
        except Exception as error:
            check(module, False, str(error))
    binary = next((ROOT / "rust/target" / profile / ("quantctl.exe" if os.name == "nt" else "quantctl")
                   for profile in ["release", "debug"]
                   if (ROOT / "rust/target" / profile / ("quantctl.exe" if os.name == "nt" else "quantctl")).is_file()), None)
    check("quantctl binary", binary is not None, str(binary or "Build quantctl during setup"))
    check("frontend built", (ROOT / "web/dist/index.html").is_file(), "Build web before Rust to embed the inspector")
    check("Git available", shutil.which("git") is not None, "Training/replay provenance requires Git and the original .git directory")
    if binary:
        try:
            result = subprocess.run([str(binary), "source-snapshot"], cwd=ROOT, capture_output=True, text=True, timeout=30)
            check("source provenance", result.returncode == 0, result.stderr.strip() or "Source snapshot available")
        except (OSError, subprocess.TimeoutExpired) as error:
            check("source provenance", False, str(error))
    print(json.dumps({"ready": not failures, "checks": checks,
                      "limits": "Checks local dependencies, not firewall isolation, data quality, model validity, or embedded asset freshness. Run scripts/verify.ps1 and supply a new config/data version."}, indent=2))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
