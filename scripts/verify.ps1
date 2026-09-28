param(
    # Only use while connected during the one-time setup.
    [switch]$InstallDependencies,
    [switch]$Benchmarks
)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
$savedEnv = @{}
foreach ($name in @('CARGO_NET_OFFLINE', 'CARGO_BUILD_JOBS', 'QUANTCTL_ALLOW_NETWORK', 'QUANTCTL_PYTHON')) {
    $savedEnv[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}
Push-Location $repoRoot
try {
    $env:CARGO_NET_OFFLINE = if ($InstallDependencies) { 'false' } else { 'true' }
    $env:CARGO_BUILD_JOBS = '2'
    $env:QUANTCTL_ALLOW_NETWORK = '0'
    $env:QUANTCTL_PYTHON = (Resolve-Path python/.venv/Scripts/python.exe).Path
    Push-Location web
    try {
        if ($InstallDependencies) {
            npm.cmd ci
            if ($LASTEXITCODE) { throw 'Frontend dependency installation failed' }
        } elseif (!(Test-Path node_modules/.bin/playwright.cmd)) {
            throw 'Missing frontend dependencies. Run npm ci and install Playwright Chromium during connected setup.'
        }
        npm.cmd test
        if ($LASTEXITCODE) { throw 'Frontend tests failed' }
        npm.cmd run build
        if ($LASTEXITCODE) { throw 'Frontend build failed' }
    } finally { Pop-Location }
    cargo fmt --manifest-path rust/Cargo.toml --all -- --check
    if ($LASTEXITCODE) { throw 'Rust formatting failed' }
    cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets --locked -- -D warnings
    if ($LASTEXITCODE) { throw 'Clippy failed' }
    cargo build --manifest-path rust/Cargo.toml --bin quantctl --locked
    if ($LASTEXITCODE) { throw 'Rust parity verifier build failed' }
    cargo test --manifest-path rust/Cargo.toml --workspace --locked
    if ($LASTEXITCODE) { throw 'Rust tests failed' }
    & $env:QUANTCTL_PYTHON -m pytest python/tests -q
    if ($LASTEXITCODE) { throw 'Python tests failed' }
    Push-Location web
    try {
        npm.cmd run contract:check
        if ($LASTEXITCODE) { throw 'API contract drift' }
        npm.cmd run test:e2e
        if ($LASTEXITCODE) { throw 'Browser tests failed' }
    } finally { Pop-Location }
    if ($Benchmarks) {
        cargo bench --manifest-path rust/Cargo.toml --locked --bench feature_benchmarks --bench simulation_benchmarks -- --test
        if ($LASTEXITCODE) { throw 'Benchmark smoke tests failed' }
    }
} finally {
    Pop-Location
    foreach ($name in $savedEnv.Keys) { [Environment]::SetEnvironmentVariable($name, $savedEnv[$name], 'Process') }
}
