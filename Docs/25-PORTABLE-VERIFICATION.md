# Portable research release verification

Verified locally on 24 October 2026. Scope: Windows x64 recorded-research app.

## Delivered behavior

`lstm-inspector.exe` embeds the production React interface, Rust ONNX inference,
the sealed `deploy_demo_v1` model, 754 synthetic daily AAPL bars, a saved backtest
and a bootstrap simulation. It opens a loopback browser URL using an available
port. Node, Rust, Python, Docker and provider keys are unnecessary to run it.

Archive and individual-file integrity are verified before serving. Each launch
uses a private temporary workspace; normal Ctrl+C shutdown removes it. The
launcher disables acquisition and ignores inherited provider credentials.
The C runtime is linked statically; inspection of PE imports found Windows system
DLLs only, with no separate Visual C++ runtime DLL requirement.

The source CLI remains available for importing datasets and training models.
The portable executable supplies a recorded sample. Real inference on synthetic
bars is not evidence of market forecasting quality.

## Fixed while verifying

- Changing the active run or the unverified-results filter preserves the current
  research route. Risk and simulation no longer jumps back to Overview.
- Model publication tolerates brief Windows access/sharing locks with bounded
  retries. It retains atomic rename, validates and seals before publication,
  preserves competing models and propagates persistent failures.

## Completed checks

- Rust formatting and full-workspace Clippy with warnings denied passed.
- Full Rust workspace suite: **193 passed**, including the new extraction tests.
- Python training, leakage, ONNX parity and publication tests: **56 passed**.
- Frontend unit tests: **13 passed**. Server-bridge tests: **7 passed**.
- API contract drift check and production frontend build passed.
- Existing browser suite plus the run-selection regression: **52 passed** on
  desktop and mobile, against a real local Rust server with deterministic fixtures.
- Release executable acceptance: **3 passed**. The binary was copied to a path
  containing spaces and launched from an empty directory with an empty PATH.
  Checks covered actual bundled ONNX forecasts, Arrow equity and simulation bands,
  nine screens on both viewports, simulation selection, disabled providers,
  no external browser requests, startup failure and verification cleanup.
- Actual Windows Ctrl+C: successful exit and temporary-workspace removal passed
  against the optimized release binary in a hidden, isolated console.

The optimized binary was rebuilt after the navigation correction, then its
portable acceptance and OS-shutdown checks passed again. The copied-executable
browser screenshots are retained under `web/test-results/standalone`.

## Release identification and remaining boundaries

`release.json` records the executable checksum, embedded archive checksum,
source revision and whether local source changes are present. The ZIP has an
adjacent SHA-256 file. The executable is unsigned. Hashes detect changes relative
to those records; they are not publisher authentication.

The new Windows Actions workflow builds and packages future artifacts. It has
been added locally; no fresh remote run or public GitHub release is claimed.

Public hosting, real-provider entitlement, live-trained model validation and
multi-user job execution remain outside this completed local release. The
older production checklist still governs those broader deployment requirements.
