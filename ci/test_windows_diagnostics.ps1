[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Controller,
    [string]$ReportScript = (Join-Path $PSScriptRoot '..\scripts\windows_ci_report.ps1')
)
$ErrorActionPreference = 'Stop'
$root = Join-Path ([IO.Path]::GetTempPath()) ('fereg-diagnostics-' + [Guid]::NewGuid().ToString('N'))
$script:OwnedProcesses = @()
$testStage = 'report_initialization'
function Assert-Test([bool]$Condition) {
    if (-not $Condition) {
        $script:FailedAssertionLine = $MyInvocation.ScriptLineNumber
        throw 'TEST_ASSERTION'
    }
}
function Assert-Rejected([scriptblock]$Operation) {
    $rejected = $false
    try { & $Operation } catch { $rejected = $true }
    Assert-Test $rejected
}
try {
    Assert-Test ($PSVersionTable.PSVersion.Major -eq 5)
    $null = New-Item -ItemType Directory -Path $root
    . $ReportScript
    Initialize-FeregWindowsReport $root
    $r = Read-FeregWindowsReport $root
    Assert-Test ($r.status -ceq 'RUNNING' -and $r.stage -ceq 'startup' -and @($r.completed).Count -eq 0)
    Publish-FeregWindowsStatus $root 0
    Assert-Test ((Get-Content (Join-Path $root 'windows-ci.status') -Raw).Trim() -ceq 'WINDOWS_CI=FAIL')
    Assert-Rejected { Set-FeregWindowsStage $root 'cargo_build' }
    Assert-Rejected { Complete-FeregWindowsReport $root }
    $testStage = 'failure_preservation'
    foreach ($stage in @('toolchain', 'source_identity', 'cargo_build')) { Set-FeregWindowsStage $root $stage }
    Set-FeregWindowsFailure $root -NativeReturnCode 101 -Reason command_failed
    Set-FeregWindowsFailure $root -StataCode 601 -Reason stata_error
    $r = Read-FeregWindowsReport $root
    Assert-Test ($r.status -ceq 'FAIL' -and $r.stage -ceq 'cargo_build' -and $r.native_rc -eq 101 -and $r.stata_rc -eq 601 -and $r.reason -ceq 'command_failed')
    Assert-Rejected { Set-FeregWindowsStage $root 'exact_fma' }
    Initialize-FeregWindowsReport $root
    $r = Read-FeregWindowsReport $root
    Assert-Test ($r.status -ceq 'RUNNING' -and @($r.completed).Count -eq 0 -and $null -eq $r.native_rc -and $null -eq $r.stata_rc)
    $testStage = 'report_completion'
    $null = New-Item -ItemType Directory -Path (Join-Path $root 'dist')
    $plugin = Join-Path $root 'dist\fereg_windows.plugin'
    [IO.File]::WriteAllBytes($plugin, [byte[]]@(1, 2, 3, 4))
    for ($i = 1; $i -lt $script:FeregWindowsStages.Count - 1; $i++) {
        Set-FeregWindowsStage $root $script:FeregWindowsStages[$i]
        $r = Read-FeregWindowsReport $root
        Assert-Test ((@($r.completed) -join ',') -ceq ($script:FeregWindowsStages[0..($i - 1)] -join ','))
    }
    Complete-FeregWindowsReport $root
    $r = Read-FeregWindowsReport $root
    Assert-Test ($r.status -ceq 'PASS' -and $r.plugin_sha256 -ceq (Get-FileHash $plugin -Algorithm SHA256).Hash.ToLowerInvariant())
    Publish-FeregWindowsStatus $root 0
    Assert-Test ((Get-Content (Join-Path $root 'windows-ci.status') -Raw).Trim() -ceq 'WINDOWS_CI=PASS')
    Assert-Rejected { Set-FeregWindowsStage $root 'startup' }
    Publish-FeregWindowsStatus $root 601
    Assert-Test ((Get-Content (Join-Path $root 'windows-ci.status') -Raw).Trim() -ceq 'WINDOWS_CI=FAIL')
    [IO.File]::WriteAllBytes($plugin, [byte[]]@(5))
    Publish-FeregWindowsStatus $root 0
    Assert-Test ((Get-Content (Join-Path $root 'windows-ci.status') -Raw).Trim() -ceq 'WINDOWS_CI=FAIL')
    Assert-Test (@(Get-ChildItem $root -Filter '*.tmp').Count -eq 0)

    $testStage = 'controller_parse'
    $controllerHash = (Get-FileHash $Controller -Algorithm SHA256).Hash
    $tokens = $null; $parseErrors = $null
    $ast = [Management.Automation.Language.Parser]::ParseFile($Controller, [ref]$tokens, [ref]$parseErrors)
    Assert-Test (@($parseErrors).Count -eq 0)
    foreach ($name in @('Invoke-StataDo', 'Remove-StataLogs')) {
        $definitions = @($ast.FindAll({ param($node)
            $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -ceq $name
        }, $true))
        Assert-Test ($definitions.Count -eq 1)
        $definition = $definitions[0].Extent.Text
        if ($name -ceq 'Invoke-StataDo') { $invokeText = $definition }
        Invoke-Expression $definition
    }
    Assert-Test ($invokeText.Contains('[ValidateRange(3000, 1800000)][int]$TimeoutMilliseconds = 900000'))
    Assert-Test ($invokeText.Contains('$watch.ElapsedMilliseconds -lt $TimeoutMilliseconds'))
    foreach ($invalidTimeout in @(0, 1800001)) {
        Assert-Rejected { Invoke-StataDo -TimeoutMilliseconds $invalidTimeout -DoFile 'unused' -WorkingDirectory $root -StatusFile 'unused' -ExpectedMarker 'unused' }
    }
    $script:FixturePath = Join-Path $root 'fixture.ps1'
    @'
param([string]$Mode)
$ErrorActionPreference = 'Stop'
$status = Join-Path $PSScriptRoot 'windows-ci.status'
switch ($Mode) {
    'pass' { [IO.File]::WriteAllText($status, "WINDOWS_CI=PASS`r`n") }
    'partial' {
        $stream = [IO.File]::Open($status, 'Create', 'Write', 'None')
        Start-Sleep -Milliseconds 500
        $stream.Dispose()
        [IO.File]::WriteAllText($status, 'WINDOWS_CI=')
        Start-Sleep -Milliseconds 500
        [IO.File]::WriteAllText($status, "WINDOWS_CI=PASS`r`n")
    }
    'fail' { [IO.File]::WriteAllText($status, "WINDOWS_CI=FAIL`r`n") }
    'missing' { exit 7 }
    'malformed' { [IO.File]::WriteAllText($status, 'INVALID') }
    'timeout' {
        $child = Start-Process -FilePath (Join-Path $PSHOME 'powershell.exe') -ArgumentList @('-NoLogo', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', ('"{0}"' -f $PSCommandPath), '-Mode', 'child') -RedirectStandardOutput (Join-Path $PSScriptRoot 'child.stdout') -RedirectStandardError (Join-Path $PSScriptRoot 'child.stderr') -PassThru
        [IO.File]::WriteAllText((Join-Path $PSScriptRoot 'child.pid'), ($child.Id.ToString() + ':' + $child.StartTime.ToUniversalTime().Ticks.ToString()))
        Start-Sleep -Seconds 30
    }
    'child' { Start-Sleep -Seconds 30 }
}
'@ | Set-Content -LiteralPath $script:FixturePath -Encoding ASCII
    function Get-StataExecutable { return (Join-Path $PSHOME 'powershell.exe') }
    function Start-Process {
        param($FilePath, $ArgumentList, $WorkingDirectory, [switch]$PassThru)
        $p = Microsoft.PowerShell.Management\Start-Process -FilePath $FilePath -ArgumentList @('-NoLogo', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', ('"{0}"' -f $script:FixturePath), '-Mode', $script:FixtureMode) -WorkingDirectory $WorkingDirectory -RedirectStandardOutput (Join-Path $WorkingDirectory 'fixture.stdout') -RedirectStandardError (Join-Path $WorkingDirectory 'fixture.stderr') -PassThru
        $script:OwnedProcesses += @(@{ Id = $p.Id; Ticks = $p.StartTime.ToUniversalTime().Ticks })
        return $p
    }
    $cases = @(@('pass', 'passed'), @('partial', 'passed'), @('fail', 'reported_failure'),
               @('missing', 'exited_without_status'), @('malformed', 'invalid_status'), @('timeout', 'timeout'))
    foreach ($case in $cases) {
        $script:FixtureMode = $case[0]
        $testStage = 'process_' + $script:FixtureMode
        $timeout = if ($script:FixtureMode -ceq 'timeout') { 3000 } else { 1800000 }
        [IO.File]::WriteAllText((Join-Path $root 'fixture.log'), 'fixture only')
        $result = Invoke-StataDo -TimeoutMilliseconds $timeout -DoFile $script:FixturePath -WorkingDirectory $root -StatusFile (Join-Path $root 'windows-ci.status') -ExpectedMarker 'WINDOWS_CI=PASS'
        Assert-Test ($result -eq ($case[1] -ceq 'passed') -and $script:StataProcess.outcome -ceq $case[1])
        Assert-Test (-not (Test-Path (Join-Path $root 'fixture.log')))
        if ($script:FixtureMode -ceq 'timeout') {
            $childFields = (Get-Content (Join-Path $root 'child.pid') -Raw).Split(':')
            $record = @{ Id = [int]$childFields[0]; Ticks = [long]$childFields[1] }
            $script:OwnedProcesses += @($record)
            $deadline = [DateTime]::UtcNow.AddSeconds(3)
            do {
                $alive = Get-Process -Id $record.Id -ErrorAction SilentlyContinue
                if ($null -eq $alive) { break }
                Start-Sleep -Milliseconds 100
            } while ([DateTime]::UtcNow -lt $deadline)
            Assert-Test ($null -eq $alive -and $script:StataProcess.elapsed_ms -ge 3000 -and $script:StataProcess.elapsed_ms -lt 15000)
        }
    }
    Assert-Test ((Get-FileHash $Controller -Algorithm SHA256).Hash -ceq $controllerHash)
    Write-Output 'WINDOWS_DIAGNOSTIC_TESTS=PASS'
} catch {
    Write-Output 'WINDOWS_DIAGNOSTIC_TESTS=FAIL'
    Write-Output ('WINDOWS_DIAGNOSTIC_TEST_STAGE=' + $testStage)
    if ($script:FailedAssertionLine) { Write-Output ('WINDOWS_DIAGNOSTIC_TEST_ASSERTION=' + $script:FailedAssertionLine) }
    $errorLine = $_.InvocationInfo.ScriptLineNumber
    $errorFile = if ($_.InvocationInfo.ScriptName -eq $ReportScript) { 'report' } else { 'test' }
    Write-Output ('WINDOWS_DIAGNOSTIC_TEST_LOCATION=' + $errorFile + '_' + $errorLine)
    $errorType = $_.Exception.GetType().Name
    if (@('RuntimeException','MethodInvocationException','ParameterBindingException','CommandNotFoundException') -contains $errorType) { Write-Output ('WINDOWS_DIAGNOSTIC_TEST_TYPE=' + $errorType) }
    exit 1
} finally {
    try {
        if (Test-Path (Join-Path $root 'child.pid')) {
            $parts = (Get-Content (Join-Path $root 'child.pid') -Raw).Split(':')
            $script:OwnedProcesses += @(@{ Id = [int]$parts[0]; Ticks = [long]$parts[1] })
        }
    } catch {}
    foreach ($record in $script:OwnedProcesses) {
        $p = Get-Process -Id $record.Id -ErrorAction SilentlyContinue
        if ($null -ne $p -and $p.StartTime.ToUniversalTime().Ticks -eq $record.Ticks) {
            Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
        }
    }
    Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
}
