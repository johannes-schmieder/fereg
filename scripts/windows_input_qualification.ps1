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
# Only fixed result lines are written; Stata logs stay on the machine.
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$ResultPath = Join-Path $Root $Result
Remove-Item -LiteralPath $ResultPath -Force -ErrorAction SilentlyContinue

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

function Invoke-Oracle {
    $stata = 'C:\Program Files\Stata19\StataMP-64.exe'
    if (-not (Test-Path -LiteralPath $stata -PathType Leaf)) { throw 'Stata/MP 19 unavailable' }
    $trace = Join-Path $Root "target\windows-input-trace-$Workers.txt"
    $marker = Join-Path $Root "target\windows-input-oracle-$Workers.done"
    Remove-Item -LiteralPath $trace, $marker -Force -ErrorAction SilentlyContinue
    $env:FEREG_SOLVER_TRACE = '1'
    $env:FEREG_SOLVER_TRACE_FILE = $trace
    if ($Workers -gt 0) {
        $env:FEREG_ORACLE_WORKERS = [string]$Workers
    } else {
        Remove-Item -LiteralPath 'Env:FEREG_ORACLE_WORKERS' -ErrorAction SilentlyContinue
    }
    $arguments = @('/q', '/e', 'do', '"scripts\windows_input_oracle.do"', ('"{0}"' -f $marker))
    $process = Start-Process -FilePath $stata -ArgumentList $arguments -WorkingDirectory $Root -PassThru
    if (-not $process.WaitForExit(900000)) {
        & taskkill.exe /PID $process.Id /T /F | Out-Null
        throw 'nested oracle timed out'
    }
    Remove-Item -LiteralPath 'Env:FEREG_SOLVER_TRACE', 'Env:FEREG_SOLVER_TRACE_FILE', 'Env:FEREG_ORACLE_WORKERS' `
        -ErrorAction SilentlyContinue
    if (-not (Test-Path -LiteralPath $marker -PathType Leaf) -or
        (Get-Content -LiteralPath $marker -Raw).Trim() -cne 'WINDOWS_INPUT_ORACLE=PASS') {
        throw 'nested oracle failed'
    }
    $summary = & python "$Root\scripts\check_input_trace.py" $trace --serial-ok ols_tiny
    if ($LASTEXITCODE -ne 0) { throw 'input digests or window use failed' }
    # The checker already fails any parallel-mode run (other than the tiny
    # sample) that did not use the window; also require the fixture's full
    # coverage (64 cases, 184 runs, as on macOS and Linux).
    $line = @($summary | Where-Object { $_ -cmatch '\Acases=(\d+) runs=(\d+) parallel_runs=(\d+)\z' })
    if ($line.Count -ne 1) { throw 'missing oracle summary' }
    $null = $line[0] -cmatch '\Acases=(\d+) runs=(\d+) parallel_runs=(\d+)\z'
    if ([int]$Matches[1] -ne 64 -or [int]$Matches[2] -ne 184 -or [int]$Matches[3] -lt 64) {
        throw 'unexpected oracle coverage'
    }
    Write-QualificationResult ("WINDOWS_INPUT_ORACLE=PASS|workers=$Workers|" + $line[0])
}

try {
    if ($Action -ceq 'witness') { Build-Witness } else { Invoke-Oracle }
    exit 0
} catch {
    Write-QualificationResult 'WINDOWS_INPUT_QUALIFICATION=FAIL'
    exit 1
}
