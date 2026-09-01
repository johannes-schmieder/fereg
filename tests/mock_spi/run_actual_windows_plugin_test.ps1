param(
    [Parameter(Mandatory = $true)]
    [string] $Plugin,

    [Parameter(Mandatory = $false)]
    [ValidateSet("candidate", "f0-baseline")]
    [string] $Mode = "candidate"
)
$ErrorActionPreference = "Stop"
$Root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$PluginPath = (Resolve-Path $Plugin).Path
$Build = Join-Path $Root "target\actual-plugin-harness-windows"
New-Item -ItemType Directory -Force -Path $Build | Out-Null
Set-Location $Build
$Loader = Join-Path $Build "actual_plugin_loader_test_windows.exe"
& cl.exe /nologo /std:c11 /O2 /MT /W4 /WX /TC /D_CRT_SECURE_NO_WARNINGS `
    /DSYSTEM=STWIN32 /I"$Root\plugin" `
    "$Root\tests\mock_spi\actual_plugin_loader_test_windows.c" "/Fe:$Loader"
if ($LASTEXITCODE -ne 0) { throw "actual Windows plugin loader compilation failed" }
& $Loader $PluginPath $Mode
if ($LASTEXITCODE -ne 0) { throw "actual Windows plugin ABI/self-test harness failed" }
