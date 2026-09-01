[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string]$Plugin,

    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string]$Checksum,

    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string]$Receipt
)

$ErrorActionPreference = "Stop"

if (Test-Path -LiteralPath $Receipt) {
    throw "verification receipt already exists"
}

$PluginItem = Get-Item -LiteralPath $Plugin
if ($PluginItem.PSIsContainer -or $PluginItem.Length -le 0) {
    throw "Windows plugin is missing or empty"
}

$ChecksumItem = Get-Item -LiteralPath $Checksum
if ($ChecksumItem.PSIsContainer -or $ChecksumItem.Length -le 0) {
    throw "Windows plugin checksum is missing or empty"
}

$ChecksumText = [System.IO.File]::ReadAllText(
    $ChecksumItem.FullName,
    [System.Text.Encoding]::ASCII
)
$ChecksumMatch = [System.Text.RegularExpressions.Regex]::Match(
    $ChecksumText,
    '\A([0-9a-f]{64})  fereg_windows\.plugin(?:\r\n|\n)?\z',
    [System.Text.RegularExpressions.RegexOptions]::CultureInvariant
)
if (-not $ChecksumMatch.Success) {
    throw "Windows plugin checksum record is malformed"
}

$ExpectedHash = $ChecksumMatch.Groups[1].Value
$ActualHash = (Get-FileHash -LiteralPath $PluginItem.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
if (-not [System.String]::Equals(
    $ExpectedHash,
    $ActualHash,
    [System.StringComparison]::Ordinal
)) {
    throw "Windows plugin checksum mismatch"
}

$ReceiptText = "WINDOWS_PLUGIN_SHA256=$ActualHash"
[System.IO.File]::WriteAllText(
    $Receipt,
    $ReceiptText,
    [System.Text.Encoding]::ASCII
)
