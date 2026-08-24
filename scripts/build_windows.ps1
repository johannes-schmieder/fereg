$ErrorActionPreference = "Stop"
$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Target = "x86_64-pc-windows-msvc"
$Build = Join-Path $Root "target\native-windows"
$Dist = Join-Path $Root "dist"
New-Item -ItemType Directory -Force -Path $Build, $Dist | Out-Null
Remove-Item -Force -ErrorAction SilentlyContinue (Join-Path $Dist "fereg_windows.plugin")

if (-not $env:FEREG_BUILD_DATE) { $env:FEREG_BUILD_DATE = (Get-Date).ToUniversalTime().ToString("yyyy-MM-dd") }
if (-not $env:FEREG_GIT_COMMIT) {
    try { $env:FEREG_GIT_COMMIT = (git -C $Root rev-parse --short=12 HEAD).Trim() }
    catch { $env:FEREG_GIT_COMMIT = "unknown" }
}
$PreviousRustFlags = $env:RUSTFLAGS
$env:RUSTFLAGS = (($PreviousRustFlags + " -C target-feature=+crt-static").Trim())

cargo build --locked --release -p fereg-plugin --target $Target
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

$StpluginObj = Join-Path $Build "stplugin.obj"
$BridgeObj = Join-Path $Build "stata_bridge.obj"
cl.exe /nologo /c /O2 /MT /W4 /WX /TC /DSYSTEM=STWIN32 /I"$Root\plugin" `
    /Fo$StpluginObj "$Root\plugin\stplugin.c"
if ($LASTEXITCODE -ne 0) { throw "stplugin.c compilation failed" }
cl.exe /nologo /c /O2 /MT /W4 /WX /TC /DSYSTEM=STWIN32 /I"$Root\plugin" `
    /Fo$BridgeObj "$Root\plugin\stata_bridge.c"
if ($LASTEXITCODE -ne 0) { throw "stata_bridge.c compilation failed" }

$RustLib = Join-Path $Root "target\$Target\release\fereg_plugin.lib"
$Plugin = Join-Path $Dist "fereg_windows.plugin"
link.exe /NOLOGO /DLL /INCREMENTAL:NO /OPT:REF /OPT:ICF `
    /OUT:$Plugin $StpluginObj $BridgeObj $RustLib `
    advapi32.lib bcrypt.lib dbghelp.lib kernel32.lib ntdll.lib userenv.lib ws2_32.lib `
    ole32.lib oleaut32.lib shell32.lib uuid.lib psapi.lib user32.lib gdi32.lib `
    winspool.lib comdlg32.lib cfgmgr32.lib synchronization.lib
if ($LASTEXITCODE -ne 0) { throw "plugin link failed" }

python "$Root\scripts\audit_binary.py" $Plugin
if ($LASTEXITCODE -ne 0) { throw "binary audit failed" }
& "$Root\tests\mock_spi\run_actual_windows_plugin_test.ps1" -Plugin $Plugin
$Hash = (Get-FileHash -Algorithm SHA256 $Plugin).Hash.ToLowerInvariant()
"$Hash  fereg_windows.plugin" | Set-Content -Encoding ascii "$Plugin.sha256"
Write-Host "Built $Plugin"
