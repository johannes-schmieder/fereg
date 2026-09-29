$ErrorActionPreference = "Stop"
$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $Root
python scripts/check_repository.py
python -m unittest discover -s tests/python -v
if (Get-Command cl.exe -ErrorAction SilentlyContinue) {
    & tests/mock_spi/run_mock_bridge_test.ps1
} else {
    Write-Host "SKIP: cl.exe not available; Windows C mock tests require Visual Studio Build Tools."
}
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
cargo test --workspace --all-targets --no-default-features
cargo test -p fereg-plugin --features ffi --lib
