[CmdletBinding()]
param(
    [ValidateSet('', 'init', 'start', 'fail', 'pass', 'status')][string]$Action = '',
    [string]$ReportRootPath = '',
    [string]$Stage = '',
    [ValidateRange(0, 99999)][int]$StataReturnCode = 0
)

# Only fixed stage/reason names, numbers and a binary hash leave the machine.
# Never copy exception messages, command output, paths or environment values.
$script:FeregWindowsStages = @(
    'startup', 'toolchain', 'source_identity', 'cargo_build', 'exact_fma',
    'fused_kernels', 'c_bridge', 'numeric_guards', 'plugin_link', 'binary_audit',
    'native_loader', 'build_complete', 'checksum', 'stata_smoke',
    'single_runtime', 'complete'
)

function Read-FeregWindowsReport {
    param([string]$ReportRoot)
    $path = Join-Path $ReportRoot 'windows-ci-diagnostics.json'
    if (-not (Test-Path -LiteralPath $path)) { return $null }
    return (Get-Content -LiteralPath $path -Raw -Encoding ASCII | ConvertFrom-Json)
}

function Write-FeregWindowsReport {
    param([string]$ReportRoot, $Report)
    $path = Join-Path $ReportRoot 'windows-ci-diagnostics.json'
    $temporary = Join-Path $ReportRoot ('windows-ci-' + [Guid]::NewGuid().ToString('N') + '.tmp')
    $backup = $temporary + '.backup.tmp'
    try {
        [IO.File]::WriteAllText($temporary, ($Report | ConvertTo-Json -Depth 4), [Text.Encoding]::ASCII)
        if (Test-Path -LiteralPath $path) {
            [IO.File]::Replace($temporary, $path, $backup)
        } else {
            [IO.File]::Move($temporary, $path)
        }
    } finally {
        Remove-Item -LiteralPath $temporary, $backup -Force -ErrorAction SilentlyContinue
    }
}

function Initialize-FeregWindowsReport {
    param([string]$ReportRoot)
    $report = [pscustomobject]@{
        schema = 'FEREG-WINDOWS-DIAGNOSTICS-V1'; status = 'RUNNING'
        stage = 'startup'; completed = @(); native_rc = $null; stata_rc = $null
        reason = 'none'; plugin_sha256 = $null
        stage_started_ms = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
    }
    Write-FeregWindowsReport $ReportRoot $report
}

function Set-FeregWindowsStage {
    param([string]$ReportRoot, [string]$Name)
    if ($script:FeregWindowsStages -cnotcontains $Name) { throw 'Invalid diagnostic stage' }
    $report = Read-FeregWindowsReport $ReportRoot
    if ($null -eq $report) { Initialize-FeregWindowsReport $ReportRoot; $report = Read-FeregWindowsReport $ReportRoot }
    if ($report.status -cne 'RUNNING') { throw 'Cannot advance a terminal diagnostic' }
    if ($report.stage -cne $Name) {
        $currentIndex = [Array]::IndexOf($script:FeregWindowsStages, [string]$report.stage)
        $nextIndex = [Array]::IndexOf($script:FeregWindowsStages, $Name)
        if ($currentIndex -lt 0 -or $nextIndex -ne $currentIndex + 1) { throw 'Diagnostic stages must be sequential' }
        $report.completed = @($report.completed) + @($report.stage)
        $report.stage = $Name
        $report.stage_started_ms = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
    }
    Write-FeregWindowsReport $ReportRoot $report
    $global:LASTEXITCODE = 0
}

function Set-FeregWindowsFailure {
    param([string]$ReportRoot, $NativeReturnCode = $null, $StataCode = $null,
          [ValidateSet('command_failed', 'check_failed', 'tool_missing', 'file_missing', 'stata_error')]
          [string]$Reason = 'check_failed')
    $report = Read-FeregWindowsReport $ReportRoot
    if ($null -eq $report) { Initialize-FeregWindowsReport $ReportRoot; $report = Read-FeregWindowsReport $ReportRoot }
    # A later Stata missing-file check must not obscure the native build failure.
    if ($report.status -cne 'FAIL') {
        $report.status = 'FAIL'
        $report.reason = $Reason
        $report.native_rc = $NativeReturnCode
    }
    if ($null -ne $StataCode) { $report.stata_rc = $StataCode }
    Write-FeregWindowsReport $ReportRoot $report
}

function Complete-FeregWindowsReport {
    param([string]$ReportRoot)
    $report = Read-FeregWindowsReport $ReportRoot
    $expected = $script:FeregWindowsStages[0..($script:FeregWindowsStages.Length - 3)] -join ','
    if ($null -eq $report -or $report.stage -cne 'single_runtime' -or
        (@($report.completed) -join ',') -cne $expected) { throw 'Cannot complete an unfinished profile' }
    Set-FeregWindowsStage $ReportRoot 'complete'
    $report = Read-FeregWindowsReport $ReportRoot
    $report.plugin_sha256 = (Get-FileHash -LiteralPath (Join-Path $ReportRoot 'dist\fereg_windows.plugin') -Algorithm SHA256).Hash.ToLowerInvariant()
    $report.status = 'PASS'
    Write-FeregWindowsReport $ReportRoot $report
}

function Publish-FeregWindowsStatus {
    param([string]$ReportRoot, [int]$StataCode)
    $report = Read-FeregWindowsReport $ReportRoot
    $marker = 'WINDOWS_CI=FAIL'
    if ($StataCode -eq 0 -and $null -ne $report -and $report.status -ceq 'PASS') {
        $expected = $script:FeregWindowsStages[0..($script:FeregWindowsStages.Length - 2)] -join ','
        $hash = (Get-FileHash -LiteralPath (Join-Path $ReportRoot 'dist\fereg_windows.plugin') -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($report.stage -ceq 'complete' -and (@($report.completed) -join ',') -ceq $expected -and
            $report.plugin_sha256 -ceq $hash) {
            $marker = 'WINDOWS_CI=PASS'
        } else {
            Set-FeregWindowsFailure $ReportRoot -Reason 'check_failed'
        }
    }
    $path = Join-Path $ReportRoot 'windows-ci.status'
    $temporary = Join-Path $ReportRoot ('windows-ci-' + [Guid]::NewGuid().ToString('N') + '.tmp')
    $backup = $temporary + '.backup.tmp'
    try {
        [IO.File]::WriteAllText($temporary, $marker + "`r`n", [Text.Encoding]::ASCII)
        if (Test-Path -LiteralPath $path) { [IO.File]::Replace($temporary, $path, $backup) }
        else { [IO.File]::Move($temporary, $path) }
    } finally {
        Remove-Item -LiteralPath $temporary, $backup -Force -ErrorAction SilentlyContinue
    }
}

if ($Action) {
    $ErrorActionPreference = 'Stop'
    if (-not $ReportRootPath) { $ReportRootPath = (Get-Location).Path }
    switch ($Action) {
        'init' { Initialize-FeregWindowsReport $ReportRootPath }
        'start' { Set-FeregWindowsStage $ReportRootPath $Stage }
        'fail' {
            if ($StataReturnCode -eq 0) { throw 'Failure needs a nonzero Stata return code' }
            Set-FeregWindowsFailure $ReportRootPath -StataCode $StataReturnCode -Reason 'stata_error'
        }
        'pass' { Complete-FeregWindowsReport $ReportRootPath }
        'status' { Publish-FeregWindowsStatus $ReportRootPath $StataReturnCode }
    }
}
