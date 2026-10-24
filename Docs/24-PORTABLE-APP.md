# Portable LSTM research app

The Windows x64 release runs without Node, Rust, Python, Docker, an API key or a
hosting account. The executable includes the production interface, Rust ONNX
runtime, an integrity-sealed model, synthetic historical AAPL bars, a backtest and
a bootstrap simulation.

## Open the app

1. Extract the release ZIP to a folder you control.
2. Double-click **lstm-inspector.exe**. It opens the forecast page in your default
   browser. Keep the console window open while using the app.
3. Choose the bundled model and dataset, then select **Generate forecast**.
4. Explore Overview, Backtest, Data and validation, Models, Signals, Risk and
   simulation, and Run activity. The evidence is embedded in the executable.
5. Press **Ctrl+C in the console** to stop the app and remove its temporary data.

If the browser does not open, copy the printed `http://127.0.0.1:.../forecast` URL.
Every launch chooses an available local port and verifies the embedded archive
and individual artifact hashes before serving. It binds only to loopback.
The app ignores inherited provider credentials and disables market acquisition.
It does not modify your research workspace. Force-killing the process or powering
off may leave a folder named `lstm-inspector-*` in your system temporary directory.

This release uses **synthetic historical data**, labelled in the forecast and
artifact evidence. It demonstrates actual inference and research inspection; it
does not supply current market predictions. The live-feed page reports disabled.
Training, importing your own datasets and authenticated hosting remain available
through the source project's CLI and deployment guide, not this sample launcher.

Optional commands, from PowerShell in the extracted folder:

```powershell
./lstm-inspector.exe --check
./lstm-inspector.exe --no-browser
./lstm-inspector.exe --port 8788
```

`--check` verifies the embedded data without running a server. The ZIP checksum
and `release.json` identify the exact executable and source revision; these are
integrity checks, not a code-signing certificate. This build is unsigned.

## Build from source

Build the frontend before compiling Rust so the embedded interface is current:

```powershell
cd web
npm ci
npm run build
cd ..
$env:RUSTFLAGS = '-C target-feature=+crt-static'
cargo build --release --locked --target x86_64-pc-windows-msvc --manifest-path rust/Cargo.toml --bin lstm-inspector
python scripts/package_standalone.py --binary rust/target/x86_64-pc-windows-msvc/release/lstm-inspector.exe --output dist/portable
```

The Windows target links its C runtime statically. Development requires the
MSVC compiler; users of the resulting release do not need it. The packaging
command refuses to overwrite an existing output directory or archive.

From `web`, run the copied-executable acceptance suite:

```powershell
$env:LSTM_INSPECTOR_BIN = (Resolve-Path ../rust/target/x86_64-pc-windows-msvc/release/lstm-inspector.exe).Path
npm run test:standalone
```

The suite clears PATH, copies the executable into a path containing spaces,
starts it from an empty directory, verifies real forecasts and Arrow records,
checks nine desktop/mobile routes, rejects inherited live-provider settings, and
checks cleanup after verification and a failed bind.
`scripts/test_standalone_shutdown.py --binary PATH_TO_EXE` additionally sends an
actual OS Ctrl+C event and verifies successful exit and temporary-data removal. Screenshots are saved under
`web/test-results/standalone`. Existing native, bridge and full research tests
remain separate checks.

The **Portable Windows App** GitHub Actions workflow builds, tests and packages
this executable. Download its `lstm-inspector-windows-x64` artifact after that
workflow succeeds. A configured workflow is not evidence that a remote run passed.

Public hosting, provider entitlement, live-trained model quality and multi-user
research execution are not established by the portable release.

[Verified local release results](25-PORTABLE-VERIFICATION.md) record the exact
completed checks and the remaining hosted/live-data boundaries.
