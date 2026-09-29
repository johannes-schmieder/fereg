$ErrorActionPreference = "Stop"
$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $Root

function Invoke-Checked {
    param([string]$Command, [string[]]$Arguments)
    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$Command exited with code $LASTEXITCODE"
    }
}

$ProfileArgs = @("--profile", "ci")
if ($env:FEREG_TEST_PROFILE) {
    $ProfileArgs = @("--profile", $env:FEREG_TEST_PROFILE)
}
Invoke-Checked cargo @("fmt", "--all", "--", "--check")
Invoke-Checked cargo (@("clippy", "--workspace", "--all-targets", "--no-default-features") + $ProfileArgs + @("--", "-D", "warnings"))
Invoke-Checked cargo (@("clippy", "-p", "fereg-plugin", "--all-targets", "--features", "ffi") + $ProfileArgs + @("--", "-D", "warnings"))
Invoke-Checked cargo (@("test", "--workspace", "--all-targets", "--no-default-features") + $ProfileArgs)
Invoke-Checked cargo (@("test", "-p", "fereg-plugin", "--features", "ffi", "--lib") + $ProfileArgs + @("--", "--test-threads=1"))
Invoke-Checked cargo (@("test", "-p", "fereg-plugin", "--features", "ffi", "--lib") + $ProfileArgs + @("--", "--ignored", "--test-threads=1"))
if (Test-Path scripts/check_sizes.sh) {
    Invoke-Checked bash @("scripts/check_sizes.sh")
}
if (Get-Command cl.exe -ErrorAction SilentlyContinue) {
    & tests/mock_spi/run_mock_bridge_test.ps1
    if ($LASTEXITCODE -ne 0) { throw "Windows mock-SPI checks failed" }
} else {
    throw "Windows mock-SPI checks require cl.exe from Visual Studio Build Tools."
}
