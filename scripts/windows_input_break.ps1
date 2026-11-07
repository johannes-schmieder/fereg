[CmdletBinding()]
param([switch]$Probe)
# A genuine GUI Break command, discovered in the owned Stata process. Its
# synchronous delivery must complete before releasing the native input window.
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Work = Join-Path $Root 'target\windows-input-break'
New-Item -ItemType Directory -Force -Path $Work | Out-Null
$Result = Join-Path $Work 'result.txt'
$Status = Join-Path $Work 'status.txt'
$Ready = Join-Path $Work 'ready.txt'
$Log = Join-Path $Work 'fixture.log'
$StataPath = 'C:\Program Files\Stata19\StataMP-64.exe'
$Token = [Guid]::NewGuid().ToString('N')
$FailureCode = 84000
$owned = $null
$temporary = $null
$passed = $false
Remove-Item -LiteralPath $Result, $Status, $Ready, $Log -Force -ErrorAction SilentlyContinue
try {
    Add-Type -Path "$PSScriptRoot/windows_input_gui.cs"
    $temporary = [FeregInputGui]::TemporaryPath()
    $env:FEREG_BREAK_MARKER_TOKEN = $Token
    $env:FEREG_PPML_BREAK_MARKER_TOKEN = $Token
    $rootPath = $Root.Replace('\', '/')
    $lines = @('version 18.0', 'clear all', 'set more off',
        ('cd "' + $rootPath + '"'),
        ('log using "' + $Log.Replace('\', '/') + '", text replace name(input_break)'),
        'assert c(os) == "Windows" & c(MP) & floor(c(stata_version)) == 19')
    if ($Probe) {
        $lines += @('program define break_probe', 'tempname ready',
            ('file open `ready'' using "' + $Ready.Replace('\', '/') + '", write replace text'),
            'file write `ready'' "READY" _n', 'file close `ready''',
            'forvalues i = 1/600 {', 'sleep 100', '}', 'end',
            'capture noisily break_probe', 'local break_rc = _rc',
            'capture assert `break_rc'' == 1', 'local fixture_rc = _rc')
    } else {
        $lines += @(
            ('sysdir set PLUS "' + $rootPath + '/target/windows-plus"'),
            ('sysdir set PERSONAL "' + $rootPath + '/target/windows-personal"'),
            'global S_ADO "UPDATES;BASE;PLUS;."',
            ('adopath ++ "' + $rootPath + '/stata"'),
            'quietly _fereg_input_host',
            'assert `"`r(interface)''"'' == "gui"',
            'findfile fereg_windows.plugin',
            'assert strpos(subinstr(`"`r(fn)''"'', char(92), "/", .), "/target/windows-plus/") > 0',
            'capture noisily do "tests/licensed_stata/estimate_input_break.do"',
            'local fixture_rc = _rc')
    }
    $lines += @('log close input_break', 'tempname result',
        ('file open `result'' using "' + $Status.Replace('\', '/') + '", write replace text'),
        'file write `result'' "rc=`fixture_rc''" _n', 'file close `result''', 'exit, clear')
    $wrapper = Join-Path $Work 'fixture.do'
    [IO.File]::WriteAllLines($wrapper, $lines)
    $FailureCode = 84001
    $owned = Start-Process -FilePath $StataPath -WorkingDirectory $Root -PassThru -ArgumentList @('/q', '/e', 'do', ('"{0}"' -f $wrapper))
    $null = $owned.Handle
    $deadline = [DateTime]::UtcNow.AddSeconds($(if ($Probe) { 45 } else { 240 }))
    $markers = if ($Probe) { [pscustomobject]@{ Path = $Ready; Expected = 'READY' } } else {
        [pscustomobject]@{ Path = (Join-Path $temporary ('fereg-native-break-' + $Token + '.marker')); Expected = 'FEREG_NATIVE_BREAK_ARMED' }
        [pscustomobject]@{ Path = (Join-Path $temporary ('feppml-native-break-' + $Token + '.marker')); Expected = 'FEPPML_NATIVE_BREAK_ARMED' }
    }
    $breaks = 0
    foreach ($entry in $markers) {
        $FailureCode = 84010 + $breaks
        while ($true) {
            if (Test-Path -LiteralPath $entry.Path) {
                $contents = ([string](Get-Content -LiteralPath $entry.Path -Raw)).Trim()
                if ($contents -ceq $entry.Expected) { break }
                if ($contents -and -not $entry.Expected.StartsWith($contents)) { throw 'invalid marker' }
            }
            if ([DateTime]::UtcNow -ge $deadline -or (Test-Path -LiteralPath $Status) -or $owned.HasExited) {
                throw 'Break marker missing'
            }
            Start-Sleep -Milliseconds 50
        }
        $FailureCode = 84020
        $delivery = [FeregInputGui]::Break([uint32]$owned.Id)
        if ($delivery -ne 0) { $FailureCode += $delivery; throw 'GUI Break unavailable' }
        if (-not $Probe -and $owned.HasExited) { throw 'Stata exited during delivery' }
        Remove-Item -LiteralPath $entry.Path -Force
        $breaks++
    }
    $FailureCode = 84030
    while ($true) {
        if (Test-Path -LiteralPath $Status) {
            $note = [string](Get-Content -LiteralPath $Status -Raw)
            if ($note -match '\Arc=\d+\r?\n\z') { break }
        }
        if ([DateTime]::UtcNow -ge $deadline -or $owned.HasExited) { throw 'fixture timed out' }
        Start-Sleep -Milliseconds 50
    }
    $FailureCode = 84031
    if ($note.Trim() -cne 'rc=0') { throw 'fixture assertion failed' }
    if (-not $Probe -and ($breaks -ne 2 -or
        -not (Select-String -LiteralPath $Log -SimpleMatch 'END_FIXTURE|estimate_input_break|rc=0' -Quiet))) {
        throw 'native fixture incomplete'
    }
    $passed = $true
} catch { $passed = $false }
finally {
    try {
        if ($null -ne $owned -and -not $owned.HasExited) {
            $owned.Kill()
            if (-not $owned.WaitForExit(10000)) { throw 'cleanup failed' }
        }
    } catch { $passed = $false; $FailureCode = 84040 }
    if ($temporary) {
        foreach ($prefix in @('fereg', 'feppml')) {
            Remove-Item -LiteralPath (Join-Path $temporary ($prefix + '-native-break-' + $Token + '.marker')) -Force -ErrorAction SilentlyContinue
        }
    }
    $final = if ($passed) {
        if ($Probe) { 'WINDOWS_INPUT_GUI=PASS' } else { 'WINDOWS_INPUT_BREAK=PASS' }
    } else { "WINDOWS_INPUT_QUALIFICATION=FAIL|code=$FailureCode" }
    [IO.File]::WriteAllText($Result, "$final`r`n")
}
if ($passed) { exit 0 } else { exit 1 }
