[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Plugin,
    [Parameter(Mandatory = $true)][string]$Checksum,
    [Parameter(Mandatory = $true)][string]$CaptureProgram,
    [Parameter(Mandatory = $true)][string]$SourceManifest,
    [Parameter(Mandatory = $true)][string]$TargetReceipt,
    [Parameter(Mandatory = $false)][string]$Capture,
    [Parameter(Mandatory = $true)][string]$Receipt
)

$ErrorActionPreference = "Stop"
if (Test-Path -LiteralPath $Receipt) { throw "capture-input receipt already exists" }

$Manifest = Get-Content -LiteralPath $SourceManifest -Raw -Encoding UTF8 |
    ConvertFrom-Json
$ControllerMatch = [regex]::Match(
    [string]$Manifest.source_revision,
    '\Acommit-([0-9a-f]{40})\z',
    [System.Text.RegularExpressions.RegexOptions]::CultureInvariant
)
if (
    $Manifest.schema_version -ne 1 -or $Manifest.project -cne "fereg" -or
    $Manifest.dirty -ne $false -or -not $ControllerMatch.Success
) { throw "Windows source manifest is not an exact clean controller" }
$ControllerCommit = $ControllerMatch.Groups[1].Value

$TargetText = [IO.File]::ReadAllText(
    (Get-Item -LiteralPath $TargetReceipt).FullName,
    [Text.Encoding]::ASCII
)
$TargetMatch = [regex]::Match(
    $TargetText,
    '\Amode=(candidate|f0-baseline)\r?\n' +
    'target_commit=([0-9a-f]{40})\r?\n' +
    'target_tree=([0-9a-f]{40})\r?\n' +
    'target_archive_sha256=([0-9a-f]{64})\r?\n' +
    'target_archive_size_bytes=([1-9][0-9]*)\r?\n' +
    'embedded_git_commit=([0-9a-f]{12})\r?\n' +
    'build_date_utc=([0-9]{4}-[0-9]{2}-[0-9]{2})\r?\n' +
    'harness_commit=([0-9a-f]{40})\r?\n' +
    'harness_tree=([0-9a-f]{40})\r?\n' +
    'FEPPML_WINDOWS_TARGET_MATERIALIZED\r?\n\z',
    [System.Text.RegularExpressions.RegexOptions]::CultureInvariant
)
if (-not $TargetMatch.Success) { throw "Windows target receipt is malformed" }
$SourceCommit = $TargetMatch.Groups[2].Value
$EmbeddedCommit = $TargetMatch.Groups[6].Value
if ($EmbeddedCommit -cne $SourceCommit.Substring(0, 12)) {
    throw "Windows target embedded commit differs from source"
}

$PluginItem = Get-Item -LiteralPath $Plugin
$ProgramItem = Get-Item -LiteralPath $CaptureProgram
if ($PluginItem.PSIsContainer -or $PluginItem.Length -le 0 -or
    $ProgramItem.PSIsContainer -or $ProgramItem.Length -le 0) {
    throw "plugin or capture program is missing or empty"
}
$ChecksumText = [IO.File]::ReadAllText(
    (Get-Item -LiteralPath $Checksum).FullName, [Text.Encoding]::ASCII
)
$ChecksumMatch = [regex]::Match(
    $ChecksumText,
    '\A([0-9a-f]{64})  fereg_windows\.plugin(?:\r\n|\n)?\z',
    [System.Text.RegularExpressions.RegexOptions]::CultureInvariant
)
if (-not $ChecksumMatch.Success) { throw "Windows plugin checksum is malformed" }
$ArtifactHash = (Get-FileHash $PluginItem.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
if ($ArtifactHash -cne $ChecksumMatch.Groups[1].Value) {
    throw "Windows plugin checksum mismatch"
}
$PluginAscii = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($PluginItem.FullName))
if (
    -not $PluginAscii.Contains($EmbeddedCommit) -or
    -not $PluginAscii.Contains($TargetMatch.Groups[7].Value)
) {
    throw "Windows plugin omits the exact embedded target commit or build date"
}
$ProgramHash = (Get-FileHash $ProgramItem.FullName -Algorithm SHA256).Hash.ToLowerInvariant()

if ($Capture) {
    $CaptureItem = Get-Item -LiteralPath $Capture
    $Document = Get-Content -LiteralPath $CaptureItem.FullName -Raw -Encoding ASCII |
        ConvertFrom-Json
    $ExpectedFields = @(
        "artifact_sha256", "artifact_size_bytes", "capture_program_sha256",
        "complete_sentinel", "embedded_git_commit", "evidence_type",
        "license_status", "loaded_plugin_sha256", "mock_only", "platform",
        "plugin_load_rc", "plugin_load_status", "schema_version",
        "scientific_claim", "source_commit", "stata_flavor", "stata_version"
    )
    $ObservedFields = @($Document.PSObject.Properties.Name | Sort-Object)
    if ([string]::Join("`n", $ObservedFields) -cne [string]::Join("`n", $ExpectedFields)) {
        throw "licensed-load capture fields are not closed"
    }
    if (
        $Document.schema_version -ne 1 -or
        $Document.evidence_type -cne "feppml-licensed-stata-load-v1" -or
        $Document.platform -cne "windows-x86_64" -or
        $Document.artifact_sha256 -cne $ArtifactHash -or
        $Document.loaded_plugin_sha256 -cne $ArtifactHash -or
        $Document.artifact_size_bytes -ne $PluginItem.Length -or
        $Document.capture_program_sha256 -cne $ProgramHash -or
        $Document.source_commit -cne $SourceCommit -or
        $Document.embedded_git_commit -cne $EmbeddedCommit -or
        $Document.license_status -cne "licensed" -or
        $Document.plugin_load_status -cne "success" -or
        $Document.plugin_load_rc -ne 0 -or $Document.mock_only -ne $false -or
        $Document.scientific_claim -ne $false -or
        $Document.complete_sentinel -cne "FEPPML_LICENSED_STATA_LOAD_COMPLETE" -or
        -not [string]$Document.stata_version -or -not [string]$Document.stata_flavor
    ) { throw "licensed-load capture differs from exact Windows inputs" }
    $CaptureHash = (Get-FileHash $CaptureItem.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    $ReceiptText = "FEPPML_PLATFORM_CAPTURE_SHA256=$CaptureHash"
}
else {
    $Lines = @(
        "controller_commit=$ControllerCommit"
        "mode=$($TargetMatch.Groups[1].Value)"
        "source_commit=$SourceCommit"
        "source_tree=$($TargetMatch.Groups[3].Value)"
        "source_archive_sha256=$($TargetMatch.Groups[4].Value)"
        "source_archive_size_bytes=$($TargetMatch.Groups[5].Value)"
        "embedded_git_commit=$EmbeddedCommit"
        "build_date_utc=$($TargetMatch.Groups[7].Value)"
        "harness_commit=$($TargetMatch.Groups[8].Value)"
        "harness_tree=$($TargetMatch.Groups[9].Value)"
        "artifact_sha256=$ArtifactHash"
        "artifact_size_bytes=$($PluginItem.Length)"
        "capture_program_sha256=$ProgramHash"
        "FEPPML_PLATFORM_CAPTURE_INPUTS_COMPLETE"
    )
    $ReceiptText = [string]::Join("`n", $Lines)
}
[IO.File]::WriteAllText($Receipt, $ReceiptText, [Text.Encoding]::ASCII)
