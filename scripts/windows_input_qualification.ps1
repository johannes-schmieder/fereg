[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][ValidateSet('witness', 'oracle')][string]$Action,
    [ValidateRange(0, 64)][int]$Workers = 0,
    [Parameter(Mandatory = $true)][ValidatePattern('\Atarget/[A-Za-z0-9_.-]+\z')][string]$Result
)

# Windows parallel numeric input qualification (scripts/windows_ci_runtime.do).
# witness: build the missing-value witness plugin with MSVC.
# oracle:  run tests/stata/development/parallel_input_oracle.do in a second
#          Stata/MP process with the solver trace enabled (the controller starts
#          the driver's Stata without it), then check every case's input digest.
# Only fixed result lines and numeric failure codes are written; Stata logs
# stay on the machine.
#
# Failure codes (the driver passes one integer to the receipt):
#   10000    Stata/MP 19 missing          20000    launch failed
#   30000+N  timeout after N case markers in the trace
#   40000    no marker, log or trace      41000+N  no marker, N case markers
#   42000    no marker, log but no trace
#   5RNNN    oracle failed with return-code class R at case marker NNN
#   6KNNN    checker failed: first failure kind K, NNN failing lines
#   70000+N  unexpected coverage (N runs) 80000    other
#   9R000    setup failed with return-code class R
# Return-code classes: 1 r(9), 2 r(198), 3 r(3499), 4 r(459), 5 r(498),
# 6 r(111), 7 r(601), 8 other below 1000, 9 other.
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$ResultPath = Join-Path $Root $Result
Remove-Item -LiteralPath $ResultPath -Force -ErrorAction SilentlyContinue
$script:FailureCode = 80000

function Write-QualificationResult([string]$Text) {
    [IO.File]::WriteAllText($ResultPath, $Text + "`r`n", [Text.Encoding]::ASCII)
}

function Initialize-Msvc {
    $vsWhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    $vsRoot = & $vsWhere -latest -products '*' `
        -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if ($LASTEXITCODE -ne 0 -or -not $vsRoot) { throw 'MSVC installation unavailable' }
    $vsDevCmd = Join-Path $vsRoot 'Common7\Tools\VsDevCmd.bat'
    $environment = & cmd.exe /d /s /c "`"$vsDevCmd`" -arch=x64 -host_arch=x64 >nul && set"
    if ($LASTEXITCODE -ne 0) { throw 'MSVC environment initialization failed' }
    foreach ($line in $environment) {
        if ($line -match '^([A-Za-z_][A-Za-z0-9_]*)=(.*)$') {
            [Environment]::SetEnvironmentVariable($Matches[1], $Matches[2], 'Process')
        }
    }
    if ($env:VSCMD_ARG_TGT_ARCH -ne 'x64') { throw 'MSVC must target x64' }
}

function Build-Witness {
    Initialize-Msvc
    $build = Join-Path $Root 'target\native-windows'
    New-Item -ItemType Directory -Force -Path $build | Out-Null
    $stplugin = Join-Path $build 'witness_stplugin.obj'
    $witness = Join-Path $build 'missing_witness.obj'
    $plugin = Join-Path $build 'missing_witness.plugin'
    Remove-Item -LiteralPath $plugin -Force -ErrorAction SilentlyContinue
    cl.exe /nologo /c /O2 /MT /W3 /TC /DSYSTEM=STWIN32 /I"$Root\plugin" `
        /Fo$stplugin "$Root\plugin\stplugin.c"
    if ($LASTEXITCODE -ne 0) { throw 'stplugin.c compilation failed' }
    cl.exe /nologo /c /O2 /MT /W3 /TC /DSYSTEM=STWIN32 /I"$Root\plugin" `
        /Fo$witness "$Root\tests\licensed_stata\missing_witness.c"
    if ($LASTEXITCODE -ne 0) { throw 'missing_witness.c compilation failed' }
    link.exe /NOLOGO /DLL /INCREMENTAL:NO /OUT:$plugin $stplugin $witness kernel32.lib
    if ($LASTEXITCODE -ne 0) { throw 'missing witness link failed' }
    Write-QualificationResult 'WINDOWS_INPUT_WITNESS_BUILD=PASS'
}

function Get-ReturnCodeClass([int]$Code) {
    switch ($Code) {
        9 { return 1 }
        198 { return 2 }
        3499 { return 3 }
        459 { return 4 }
        498 { return 5 }
        111 { return 6 }
        601 { return 7 }
    }
    if ($Code -lt 1000) { return 8 }
    return 9
}

function Get-CaseMarkers([string]$Trace) {
    if (-not (Test-Path -LiteralPath $Trace -PathType Leaf)) { return -1 }
    $count = @(Get-Content -LiteralPath $Trace |
        Where-Object { $_ -clike 'FEREG_SOLVER_TRACE|phase=case|*' }).Count
    return [Math]::Min($count, 999)
}

function Invoke-Oracle {
    $stata = 'C:\Program Files\Stata19\StataMP-64.exe'
    if (-not (Test-Path -LiteralPath $stata -PathType Leaf)) {
        $script:FailureCode = 10000
        throw 'Stata/MP 19 unavailable'
    }
    $trace = Join-Path $Root "target\windows-input-trace-$Workers.txt"
    $marker = Join-Path $Root "target\windows-input-oracle-$Workers.done"
    $log = Join-Path $Root 'windows_input_oracle.log'
    Remove-Item -LiteralPath $trace, $marker, $log -Force -ErrorAction SilentlyContinue
    $env:FEREG_SOLVER_TRACE = '1'
    $env:FEREG_SOLVER_TRACE_FILE = $trace
    if ($Workers -gt 0) {
        $env:FEREG_ORACLE_WORKERS = [string]$Workers
    } else {
        Remove-Item -LiteralPath 'Env:FEREG_ORACLE_WORKERS' -ErrorAction SilentlyContinue
    }
    $arguments = @('/q', '/e', 'do', '"scripts\windows_input_oracle.do"', ('"{0}"' -f $marker))
    $script:FailureCode = 20000
    $process = Start-Process -FilePath $stata -ArgumentList $arguments -WorkingDirectory $Root -PassThru
    if (-not $process.WaitForExit(900000)) {
        & taskkill.exe /PID $process.Id /T /F | Out-Null
        $script:FailureCode = 30000 + [Math]::Max(0, (Get-CaseMarkers $trace))
        throw 'nested oracle timed out'
    }
    Remove-Item -LiteralPath 'Env:FEREG_SOLVER_TRACE', 'Env:FEREG_SOLVER_TRACE_FILE', 'Env:FEREG_ORACLE_WORKERS' `
        -ErrorAction SilentlyContinue
    $markers = Get-CaseMarkers $trace
    if (-not (Test-Path -LiteralPath $marker -PathType Leaf)) {
        if ($markers -ge 0) { $script:FailureCode = 41000 + $markers }
        elseif (Test-Path -LiteralPath $log -PathType Leaf) { $script:FailureCode = 42000 }
        else { $script:FailureCode = 40000 }
        throw 'nested oracle wrote no marker'
    }
    $script:FailureCode = 80000
    $note = (Get-Content -LiteralPath $marker -Raw).Trim()
    if ($note -cmatch '\AWINDOWS_INPUT_ORACLE=FAIL\|stage=([12])\|rc=(\d+)\z') {
        $stage = $Matches[1]
        $class = Get-ReturnCodeClass ([int]$Matches[2])
        if ($stage -ceq '1') { $script:FailureCode = 90000 + 1000 * $class }
        else { $script:FailureCode = 50000 + 1000 * $class + [Math]::Max(0, $markers) }
        throw 'nested oracle failed'
    }
    if ($note -cne 'WINDOWS_INPUT_ORACLE=PASS') { throw 'invalid oracle marker' }
    $summary = @(& python "$Root\scripts\check_input_trace.py" $trace --serial-ok ols_tiny)
    if ($LASTEXITCODE -ne 0) {
        $failures = @($summary | Where-Object { $_ -clike 'FAIL *' })
        $kind = 0
        if ($failures.Count -eq 0) { $kind = 5 }
        elseif ($failures[0] -clike '*distinct input digests*') { $kind = 1 }
        elseif ($failures[0] -clike '*the numeric window did not run') { $kind = 2 }
        elseif ($failures[0] -clike '*the numeric window ran') { $kind = 3 }
        elseif ($failures[0] -clike '*no input digest') { $kind = 4 }
        $script:FailureCode = 60000 + 1000 * $kind + [Math]::Min($failures.Count, 999)
        throw 'input digests or window use failed'
    }
    # The checker already fails any parallel-mode run (other than the tiny
    # sample) that did not use the window; also require the fixture's full
    # coverage (70 cases, 202 runs, as on macOS and Linux).
    $line = @($summary | Where-Object { $_ -cmatch '\Acases=(\d+) runs=(\d+) parallel_runs=(\d+)\z' })
    if ($line.Count -ne 1) { throw 'missing oracle summary' }
    $null = $line[0] -cmatch '\Acases=(\d+) runs=(\d+) parallel_runs=(\d+)\z'
    if ([int]$Matches[1] -ne 70 -or [int]$Matches[2] -ne 202 -or [int]$Matches[3] -lt 70) {
        $script:FailureCode = 70000 + [Math]::Min([int]$Matches[2], 999)
        throw 'unexpected oracle coverage'
    }
    Write-QualificationResult ("WINDOWS_INPUT_ORACLE=PASS|workers=$Workers|" + $line[0])
}

try {
    if ($Action -ceq 'witness') { Build-Witness } else { Invoke-Oracle }
    exit 0
} catch {
    Write-QualificationResult ('WINDOWS_INPUT_QUALIFICATION=FAIL|code=' + $script:FailureCode)
    exit 1
}
