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
