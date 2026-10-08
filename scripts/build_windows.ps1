[CmdletBinding()]
param(
    [Parameter(Mandatory = $false)]
    [string]$SourceRoot,

    [Parameter(Mandatory = $false)]
    [string]$ExpectedSourceCommit,

    [Parameter(Mandatory = $false)]
    [string]$ExpectedBuildDate
)

$ErrorActionPreference = "Stop"
# The private controller uses ordinary PowerShell, not a VS developer shell.
# Cargo can locate MSVC itself; the C bridge and dumpbin also need its SDK paths.
$VsWhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
$VsRoot = & $VsWhere -latest -products '*' `
    -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if ($LASTEXITCODE -ne 0 -or -not $VsRoot) { throw "MSVC installation unavailable" }
$VsDevCmd = Join-Path $VsRoot "Common7\Tools\VsDevCmd.bat"
$MsvcEnvironment = & cmd.exe /d /s /c "`"$VsDevCmd`" -arch=x64 -host_arch=x64 >nul && set"
if ($LASTEXITCODE -ne 0) { throw "MSVC environment initialization failed" }
foreach ($MsvcLine in $MsvcEnvironment) {
    if ($MsvcLine -match '^([A-Za-z_][A-Za-z0-9_]*)=(.*)$') {
        [Environment]::SetEnvironmentVariable($Matches[1], $Matches[2], 'Process')
    }
}
if ($env:VSCMD_ARG_TGT_ARCH -ne 'x64') { throw "MSVC must target x64" }
foreach ($MsvcTool in @('cl.exe', 'link.exe', 'dumpbin.exe')) {
    Get-Command $MsvcTool -CommandType Application -ErrorAction Stop | Out-Null
}
$ControllerRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Root = if ($SourceRoot) {
    (Resolve-Path -LiteralPath $SourceRoot).Path
} else {
    $ControllerRoot
}
$Target = "x86_64-pc-windows-msvc"
$Build = Join-Path $Root "target\native-windows"
$Dist = Join-Path $Root "dist"
New-Item -ItemType Directory -Force -Path $Build, $Dist | Out-Null
Remove-Item -Force -ErrorAction SilentlyContinue (Join-Path $Dist "fereg_windows.plugin")

if ($SourceRoot) {
    if ($ExpectedSourceCommit -cnotmatch '\A[0-9a-f]{40}\z') {
        throw "materialized Windows builds require an exact full target commit"
    }
    $ExpectedEmbeddedCommit = $ExpectedSourceCommit.Substring(0, 12)
    if ($env:FEREG_GIT_COMMIT -and $env:FEREG_GIT_COMMIT -cne $ExpectedEmbeddedCommit) {
        throw "pre-existing embedded commit differs from the exact target"
    }
    $env:FEREG_GIT_COMMIT = $ExpectedEmbeddedCommit
    if ($ExpectedBuildDate -cnotmatch '\A[0-9]{4}-[0-9]{2}-[0-9]{2}\z') {
        throw "materialized Windows builds require the target commit UTC date"
    }
    if ($env:FEREG_BUILD_DATE -and $env:FEREG_BUILD_DATE -cne $ExpectedBuildDate) {
        throw "pre-existing build date differs from the exact target commit"
    }
    $env:FEREG_BUILD_DATE = $ExpectedBuildDate
}
elseif (
    $ExpectedSourceCommit -or $ExpectedBuildDate
) {
    throw "expected source identity requires SourceRoot"
}
else {
    if (-not $env:FEREG_BUILD_DATE) {
        $env:FEREG_BUILD_DATE = (Get-Date).ToUniversalTime().ToString("yyyy-MM-dd")
    }
    if (-not $env:FEREG_GIT_COMMIT) {
        $WindowsSourceManifest = Join-Path $Root "windows-ci-source.json"
        if (Test-Path -LiteralPath $WindowsSourceManifest) {
            $Manifest = Get-Content -LiteralPath $WindowsSourceManifest -Raw -Encoding UTF8 |
                ConvertFrom-Json
            $SourceMatch = [System.Text.RegularExpressions.Regex]::Match(
                [string]$Manifest.source_revision,
                '\Acommit-([0-9a-f]{40})\z',
                [System.Text.RegularExpressions.RegexOptions]::CultureInvariant
            )
            if (
                $Manifest.schema_version -ne 1 -or
                -not [System.String]::Equals(
                    [string]$Manifest.project,
                    "fereg",
                    [System.StringComparison]::Ordinal
                ) -or
                $Manifest.dirty -ne $false -or
                -not $SourceMatch.Success
            ) {
                throw "Windows source manifest is not an exact clean fereg commit"
            }
            $env:FEREG_GIT_COMMIT = $SourceMatch.Groups[1].Value.Substring(0, 12)
        }
        else {
            try { $env:FEREG_GIT_COMMIT = (git -C $Root rev-parse --short=12 HEAD).Trim() }
            catch { throw "cannot derive an exact Windows source commit" }
        }
    }
}
if ($env:FEREG_GIT_COMMIT -cnotmatch '\A[0-9a-f]{12}\z') {
    throw "embedded Windows source commit must be exactly 12 lowercase hex characters"
}
if ($env:FEREG_BUILD_DATE -cnotmatch '\A[0-9]{4}-[0-9]{2}-[0-9]{2}\z') {
    throw "Windows build date must be an exact ISO date"
}
$PreviousRustFlags = $env:RUSTFLAGS
$env:RUSTFLAGS = (($PreviousRustFlags + " -C target-feature=+crt-static").Trim())

cargo build --manifest-path "$Root\Cargo.toml" --locked --release `
    -p fereg-plugin --target $Target
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

# Exercise the baseline UCRT mul_add path, including true double-rounding
# witnesses, before accepting the runtime-FMA bit-identity assumption.
# Capture stdout only: PowerShell 5.1 can treat redirected native stderr as
# terminating errors even for Cargo's ordinary compilation progress.
$ArithmeticOutput = & cargo test --manifest-path "$Root\Cargo.toml" --locked `
    --profile ci -p fereg-core --target $Target --lib `
    kernels::tests::platform_mul_add_is_exactly_rounded -- --exact
if ($LASTEXITCODE -ne 0 -or
    -not (($ArithmeticOutput -join "`n") -match 'test result: ok\. 1 passed; 0 failed;')) {
    $ArithmeticOutput | ForEach-Object { Write-Host $_ }
    throw "Windows exact-FMA arithmetic check failed or did not run"
}
Write-Host "Windows exact-FMA arithmetic check: PASS"

$KernelOutput = & cargo test --manifest-path "$Root\Cargo.toml" --locked `
    --profile ci -p fereg-plugin --target $Target --lib `
    fused_kernels::x86_64::tests -- --show-output
if ($LASTEXITCODE -ne 0 -or
    -not (($KernelOutput -join "`n") -match 'test result: ok\. 3 passed; 0 failed;')) {
    $KernelOutput | ForEach-Object { Write-Host $_ }
    throw "Windows fused-kernel checks failed or did not run"
}
Write-Host "Windows fused-kernel checks: PASS"

$StpluginObj = Join-Path $Build "stplugin.obj"
$BridgeObj = Join-Path $Build "stata_bridge.obj"
cl.exe /nologo /c /O2 /MT /W4 /WX /TC /DSYSTEM=STWIN32 /I"$Root\plugin" `
    /Fo$StpluginObj "$Root\plugin\stplugin.c"
if ($LASTEXITCODE -ne 0) { throw "stplugin.c compilation failed" }
cl.exe /nologo /c /O2 /MT /W4 /WX /TC /DSYSTEM=STWIN32 /I"$Root\plugin" `
    /Fo$BridgeObj "$Root\plugin\stata_bridge.c"
if ($LASTEXITCODE -ne 0) { throw "stata_bridge.c compilation failed" }

$GuardObj = Join-Path $Build "numeric_batch_guard_test.obj"
$GuardExe = Join-Path $Build "numeric_batch_guard_test.exe"
cl.exe /nologo /c /std:c11 /O2 /MT /W4 /WX /TC /DSYSTEM=STWIN32 /I"$Root\plugin" `
    /Fo$GuardObj "$Root\tests\mock_spi\numeric_batch_guard_test.c"
if ($LASTEXITCODE -ne 0) { throw "numeric batch guard compilation failed" }
link.exe /NOLOGO /OUT:$GuardExe $StpluginObj $BridgeObj $GuardObj kernel32.lib
if ($LASTEXITCODE -ne 0) { throw "numeric batch guard link failed" }
$GuardOutput = & $GuardExe
if ($LASTEXITCODE -ne 0 -or
    $GuardOutput -cnotcontains 'INPUT_GUARD_PASS' -or
    $GuardOutput -cnotcontains 'OUTPUT_GUARD_PASS') {
    throw "Windows numeric batch guard check failed or did not run"
}
Write-Host "Windows numeric batch guard check: PASS"

$RustLib = Join-Path $Root "target\$Target\release\fereg_plugin.lib"
$Plugin = Join-Path $Dist "fereg_windows.plugin"
link.exe /NOLOGO /DLL /INCREMENTAL:NO /OPT:REF /OPT:ICF `
    /OUT:$Plugin $StpluginObj $BridgeObj $RustLib `
    advapi32.lib bcrypt.lib dbghelp.lib kernel32.lib ntdll.lib userenv.lib ws2_32.lib `
    ole32.lib oleaut32.lib shell32.lib uuid.lib psapi.lib user32.lib gdi32.lib `
    winspool.lib comdlg32.lib cfgmgr32.lib synchronization.lib
if ($LASTEXITCODE -ne 0) { throw "plugin link failed" }

$PluginAscii = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($Plugin))
if (
    -not $PluginAscii.Contains($env:FEREG_GIT_COMMIT) -or
    -not $PluginAscii.Contains($env:FEREG_BUILD_DATE)
) { throw "Windows plugin omits its exact source commit or frozen build date" }

python "$ControllerRoot\scripts\audit_binary.py" $Plugin
if ($LASTEXITCODE -ne 0) { throw "binary audit failed" }
& "$ControllerRoot\tests\mock_spi\run_actual_windows_plugin_test.ps1" `
    -Plugin $Plugin
$Hash = (Get-FileHash -Algorithm SHA256 $Plugin).Hash.ToLowerInvariant()
"$Hash  fereg_windows.plugin" | Set-Content -Encoding ascii "$Plugin.sha256"
Write-Host "Built $Plugin"
