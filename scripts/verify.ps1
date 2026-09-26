$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
Push-Location $repoRoot
try {
    cargo fmt --manifest-path rust/Cargo.toml --all -- --check
    if ($LASTEXITCODE) { throw 'Rust formatting failed' }
    cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings
    if ($LASTEXITCODE) { throw 'Clippy failed' }
    cargo test --manifest-path rust/Cargo.toml --workspace
    if ($LASTEXITCODE) { throw 'Rust tests failed' }
    & python/.venv/Scripts/python.exe -m pytest python/tests -q
    if ($LASTEXITCODE) { throw 'Python tests failed' }
    Push-Location web
    try {
        npm.cmd ci
        if ($LASTEXITCODE) { throw 'Frontend dependency installation failed' }
        npm.cmd run contract:check
        if ($LASTEXITCODE) { throw 'API contract drift' }
        npm.cmd test
        if ($LASTEXITCODE) { throw 'Frontend tests failed' }
        npm.cmd run build
        if ($LASTEXITCODE) { throw 'Frontend build failed' }
        npm.cmd run test:e2e
        if ($LASTEXITCODE) { throw 'Browser tests failed' }
    } finally { Pop-Location }
} finally { Pop-Location }
