[CmdletBinding()]
param([switch]$Worker, [switch]$Probe,
    [ValidatePattern('\A[0-9a-f]{32}\z')][string]$Token)
# Stata's documented Windows Automation Break, including an active native
# numeric window. No GUI keystrokes, guessed command IDs or synthetic SPI flag.
# https://www.stata.com/automation/ (sections 3.6-3.8)
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Work = Join-Path $Root 'target\windows-input-break'
$Identity = Join-Path $Work 'identity.json'
$Result = Join-Path $Work 'result.txt'
$WorkerResult = Join-Path $Work 'worker-result.txt'
$Stage = Join-Path $Work 'stage.txt'
$Status = Join-Path $Work 'status.txt'
$StataPath = 'C:\Program Files\Stata19\StataMP-64.exe'
$FailureCode = 81999

if ($Probe -and -not $Worker) {
    New-Item -ItemType Directory -Force -Path $Work | Out-Null
    try {
        # The installed application does not register Automation by default.
        # Use Stata's documented setup switch only when its ProgID is absent;
        # never replace a registration pointing at another installed edition.
        if (-not (Test-Path -LiteralPath 'Registry::HKEY_CLASSES_ROOT\stata.StataOLEApp\CLSID')) {
            $FailureCode = 81020
            $registration = Start-Process -FilePath $StataPath -ArgumentList '/Register' -PassThru
            $null = $registration.Handle
            if (-not $registration.WaitForExit(30000)) {
                $registration.Kill()
                $registration.WaitForExit()
                throw 'automation registration timed out'
            }
            if ($registration.ExitCode -ne 0) { throw 'automation registration failed' }
        }
        $FailureCode = 81010
        $class = (Get-Item -LiteralPath 'Registry::HKEY_CLASSES_ROOT\stata.StataOLEApp\CLSID').GetValue('')
        $FailureCode = 81011
        $key = Get-Item -LiteralPath ('Registry::HKEY_CLASSES_ROOT\CLSID\' + $class + '\LocalServer32')
        $server = [string]$key.GetValue('')
        $FailureCode = 81012
        if ($server -notmatch '^"?C:\\Program Files\\Stata19\\StataMP-64\.exe"?(?:\s.*)?$') {
            throw 'wrong automation server'
        }
    } catch {
        [IO.File]::WriteAllText($Result, "WINDOWS_INPUT_QUALIFICATION=FAIL|code=$FailureCode`r`n")
        exit 1
    }
}

function Get-OwnedStata {
    if (-not (Test-Path -LiteralPath $Identity)) { return $null }
    $record = Get-Content -LiteralPath $Identity -Raw | ConvertFrom-Json
    $candidate = Get-Process -Id $record.pid -ErrorAction SilentlyContinue
    if ($null -eq $candidate) { return $null }
    if ($candidate.Path -ine $StataPath -or
        $candidate.StartTime.ToUniversalTime().Ticks -ne $record.ticks -or
        $candidate.SessionId -ne $record.session -or
        $candidate.SessionId -ne (Get-Process -Id $PID).SessionId) {
        throw 'automation process identity mismatch'
    }
    return $candidate
}

if (-not $Worker) {
    New-Item -ItemType Directory -Force -Path $Work | Out-Null
    Remove-Item -LiteralPath $Identity, $Result, $WorkerResult, $Status, $Stage -Force -ErrorAction SilentlyContinue
    $Token = [Guid]::NewGuid().ToString('N')
    $expectedResult = if ($Probe) { 'WINDOWS_INPUT_AUTOMATION=PASS' } else { 'WINDOWS_INPUT_BREAK=PASS' }
    $child = $null
    $passed = $false
    $activationCode = 0
    $initialStata = @(Get-Process -Name StataMP-64 -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
    try {
        $childArguments = @(
            '-NoLogo', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass',
            '-File', ('"{0}"' -f $PSCommandPath), '-Worker', '-Token', $Token)
        if ($Probe) { $childArguments += '-Probe' }
        $child = Start-Process powershell.exe -PassThru -WorkingDirectory $Root -ArgumentList $childArguments
        $null = $child.Handle
        $limit = if ($Probe) { 60000 } else { 300000 }
        if (-not $child.WaitForExit($limit)) {
            if ($Probe -and (Test-Path -LiteralPath $Stage) -and
                ([string](Get-Content -LiteralPath $Stage -Raw)) -ceq '81001') {
                $activationCode = & "$PSScriptRoot/windows_input_com_state.ps1" -Before $initialStata
            }
            throw 'automation timeout'
        }
        $child.Refresh()
        if ($child.ExitCode -ne 0 -or -not (Test-Path -LiteralPath $WorkerResult) -or
            (Get-Content -LiteralPath $WorkerResult -Raw).Trim() -cne $expectedResult) {
            throw 'automation fixture failed'
        }
        $passed = $true
    } catch {
        $passed = $false
    } finally {
        try {
            $owned = Get-OwnedStata
            if ($null -ne $owned) {
                if (-not $owned.HasExited) { $owned.Kill() }
                if (-not $owned.WaitForExit(10000)) { throw 'automation cleanup failed' }
            }
        } catch { $passed = $false }
        try {
            if ($null -ne $child -and -not $child.HasExited) {
                & taskkill.exe /PID $child.Id /T /F | Out-Null
                if (-not $child.WaitForExit(10000)) { throw 'worker cleanup failed' }
            }
        } catch { $passed = $false }
        try {
            if (Test-Path -LiteralPath $Identity) {
                $temporary = [string](Get-Content -LiteralPath $Identity -Raw | ConvertFrom-Json).temporary
                foreach ($prefix in @('fereg', 'feppml')) {
                    Remove-Item -LiteralPath (Join-Path $temporary ($prefix + '-native-break-' + $Token + '.marker')) -Force -ErrorAction SilentlyContinue
                }
            } else { $passed = $false }
        } catch { $passed = $false }
        if (-not $passed) {
            $failure = if (Test-Path -LiteralPath $WorkerResult) { (Get-Content -LiteralPath $WorkerResult -Raw).Trim() } else { '' }
            if ($failure -cnotmatch '\AWINDOWS_INPUT_QUALIFICATION=FAIL\|code=81[0-9]{3}\z') {
                $timeoutCode = 81999
                if (Test-Path -LiteralPath $Stage) {
                    $last = [string](Get-Content -LiteralPath $Stage -Raw)
                    if ($last -cmatch '\A810[0-9]{2}\z') { $timeoutCode = [int]$last + 1000 }
                }
                if ($activationCode -ge 83000 -and $activationCode -le 83334) { $timeoutCode = $activationCode }
                $failure = "WINDOWS_INPUT_QUALIFICATION=FAIL|code=$timeoutCode"
            }
            [IO.File]::WriteAllText($Result, "$failure`r`n")
        } else { [IO.File]::WriteAllText($Result, "$expectedResult`r`n") }
    }
    if ($passed) { exit 0 } else { exit 1 }
}

$automation = $null
$code = 1
try {
    # SCM starts COM servers with its environment. Do not rely on inheritance
    # of FEREG_* from PowerShell: only the hidden adapters' token binding is
    # replaced in two generated test copies, leaving runtime behavior intact.
    if (-not $Token) { throw 'missing invocation token' }
    $adapters = Join-Path $Work 'ado'
    New-Item -ItemType Directory -Force -Path $adapters | Out-Null
    foreach ($entry in @(
        @('_fereg_estimate_ols.ado', 'FEREG_BREAK_MARKER_TOKEN'),
        @('_fereg_estimate_ppml.ado', 'FEREG_PPML_BREAK_MARKER_TOKEN'))) {
        $source = [IO.File]::ReadAllText((Join-Path $Root ('stata\' + $entry[0])))
        $anchor = 'local break_marker_token : environment ' + $entry[1]
        if (($source.Split(@($anchor), [StringSplitOptions]::None)).Count -ne 2) {
            throw 'hidden adapter anchor changed'
        }
        [IO.File]::WriteAllText((Join-Path $adapters $entry[0]),
            $source.Replace($anchor, ('local break_marker_token "' + $token + '"')))
    }
    $FailureCode = 81001
    [IO.File]::WriteAllText($Stage, '81001')
    $automation = New-Object -ComObject stata.StataOLEApp
    $FailureCode = 81002
    [IO.File]::WriteAllText($Stage, '81002')
    $setup = @"
shell powershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PSScriptRoot\windows_input_com_identity.ps1" -Output "$Identity"
"@
    if ($automation.DoCommand($setup.Trim()) -ne 0) { throw 'identity command failed' }
    $owned = Get-OwnedStata
    if ($null -eq $owned) { throw 'automation identity absent' }
    if ($Probe) {
        [IO.File]::WriteAllText($Stage, '81021')
        if ($automation.DoCommand('assert c(os) == "Windows" & c(MP) & floor(c(stata_version)) == 19') -ne 0) {
            throw 'automation host mismatch'
        }
        $probeFile = Join-Path $Work 'probe.do'
        [IO.File]::WriteAllLines($probeFile, @('tempname result',
            ('file open `result'' using "' + $Status.Replace('\', '/') + '", write replace text'),
            'file write `result'' "rc=0" _n', 'file close `result'''))
        [IO.File]::WriteAllText($Stage, '81022')
        if ($automation.DoCommandAsync(('do "' + $probeFile.Replace('\', '/') + '"')) -ne 0) { throw 'probe launch failed' }
        $probeDeadline = [DateTime]::UtcNow.AddSeconds(20)
        while ([DateTime]::UtcNow -lt $probeDeadline) {
            if ((Test-Path -LiteralPath $Status) -and
                ([string](Get-Content -LiteralPath $Status -Raw)) -match '\Arc=0\r?\n\z') {
                [IO.File]::WriteAllText($WorkerResult, "WINDOWS_INPUT_AUTOMATION=PASS`r`n")
                $code = 0
                break
            }
            Start-Sleep -Milliseconds 50
        }
        if ($code -ne 0) { throw 'probe timed out' }
    } else {
    $FailureCode = 81003
    $temporary = [string](Get-Content -LiteralPath $Identity -Raw | ConvertFrom-Json).temporary
    if (-not $temporary -or -not (Test-Path -LiteralPath $temporary -PathType Container)) {
        throw 'temporary path unavailable'
    }
    $wrapper = Join-Path $Work 'fixture.do'
    $rootPath = $Root.Replace('\', '/')
    $adapterPath = $adapters.Replace('\', '/')
    $statusPath = $Status.Replace('\', '/')
    $log = Join-Path $Work 'fixture.log'
    $lines = @(
        'version 18.0', 'clear all', 'set more off',
        ('cd "' + $rootPath + '"'),
        ('log using "' + $log.Replace('\', '/') + '", text replace name(input_break)'),
        ('sysdir set PLUS "' + $rootPath + '/target/windows-plus"'),
        ('sysdir set PERSONAL "' + $rootPath + '/target/windows-personal"'),
        'global S_ADO "UPDATES;BASE;PLUS;."',
        ('adopath ++ "' + $rootPath + '/stata"'),
        ('adopath ++ "' + $adapterPath + '"'),
        'assert c(os) == "Windows" & c(MP) & floor(c(stata_version)) == 19',
        'quietly _fereg_input_host',
        'assert `"`r(interface)''"'' == "gui"',
        'findfile fereg_windows.plugin',
        'assert strpos(subinstr(`"`r(fn)''"'', char(92), "/", .), "/target/windows-plus/") > 0',
        'capture noisily do "tests/licensed_stata/estimate_input_break.do"',
        'local fixture_rc = _rc',
        'log close input_break',
        'tempname result',
        ('file open `result'' using "' + $statusPath + '", write replace text'),
        'file write `result'' "rc=`fixture_rc''" _n',
        'file close `result''')
    [IO.File]::WriteAllLines($wrapper, $lines)
    $FailureCode = 81004
    if ($automation.DoCommandAsync(('do "' + $wrapper.Replace('\', '/') + '"')) -ne 0) {
        throw 'asynchronous fixture launch failed'
    }
    $deadline = [DateTime]::UtcNow.AddSeconds(240)
    $breaks = 0
    foreach ($prefix in @('fereg', 'feppml')) {
        $FailureCode = 81005 + 10 * $breaks
        $marker = Join-Path $temporary ($prefix + '-native-break-' + $token + '.marker')
        $expected = if ($prefix -ceq 'fereg') { 'FEREG_NATIVE_BREAK_ARMED' } else { 'FEPPML_NATIVE_BREAK_ARMED' }
        while ($true) {
            if (Test-Path -LiteralPath $marker) {
                $contents = ([string](Get-Content -LiteralPath $marker -Raw)).Trim()
                if ($contents -ceq $expected) { break }
                if ($contents -and -not $expected.StartsWith($contents)) { throw 'invalid native marker' }
            }
            if ([DateTime]::UtcNow -ge $deadline -or (Test-Path -LiteralPath $Status) -or $owned.HasExited) {
                throw 'active-window marker missing'
            }
            Start-Sleep -Milliseconds 50
        }
        $FailureCode = 81006 + 10 * $breaks
        $delivery = [Diagnostics.Stopwatch]::StartNew()
        $automation.UtilSetStataBreak()
        # A marshalled call that returns only after the native 120-second park
        # expires is not evidence of Break inside the active numeric window.
        if ($delivery.Elapsed.TotalSeconds -gt 10) { throw 'Break delivery stalled' }
        Remove-Item -LiteralPath $marker -Force
        $breaks++
    }
    $FailureCode = 81007
    $note = ''
    while ($true) {
        if (Test-Path -LiteralPath $Status) {
            $note = [string](Get-Content -LiteralPath $Status -Raw)
            if ($note -match '\Arc=\d+\r?\n\z') { break }
        }
        if ([DateTime]::UtcNow -ge $deadline -or $owned.HasExited) { throw 'fixture timed out' }
        Start-Sleep -Milliseconds 50
    }
    if ($breaks -ne 2 -or $note.Trim() -cne 'rc=0' -or
        -not (Select-String -LiteralPath $log -SimpleMatch 'END_FIXTURE|estimate_input_break|rc=0' -Quiet)) {
        throw 'fixture assertion failed'
    }
    [IO.File]::WriteAllText($WorkerResult, "WINDOWS_INPUT_BREAK=PASS`r`n")
    $code = 0
    }
} catch {
    # Only this fixed code is returned; COM errors can contain local details.
    [IO.File]::WriteAllText($WorkerResult, "WINDOWS_INPUT_QUALIFICATION=FAIL|code=$FailureCode`r`n")
} finally {
    [IO.File]::WriteAllText($Stage, '81009')
    if ($null -ne $automation) { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($automation) }
}
exit $code
