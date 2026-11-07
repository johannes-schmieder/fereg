# Called only by the new automation Stata, before any test starts. Bind the
# cleanup handle to that caller rather than guessing from a process-name list.
param([Parameter(Mandatory=$true)][string]$Output)
$ErrorActionPreference = 'Stop'
$process = Get-CimInstance Win32_Process -Filter "ProcessId=$PID"
for ($depth = 0; $depth -lt 6; $depth++) {
    $process = Get-CimInstance Win32_Process -Filter ("ProcessId=" + $process.ParentProcessId)
    if ($null -eq $process) { throw 'missing caller' }
    if ($process.ExecutablePath -ieq 'C:\Program Files\Stata19\StataMP-64.exe') {
        $identity = [ordered]@{
            pid = [int]$process.ProcessId
            ticks = (Get-Process -Id $process.ProcessId).StartTime.ToUniversalTime().Ticks
            session = [int]$process.SessionId
            temporary = [IO.Path]::GetTempPath()
        }
        $identity | ConvertTo-Json -Compress | Set-Content -LiteralPath $Output -Encoding ASCII
        exit 0
    }
    if ($process.Name -notin @('cmd.exe', 'powershell.exe')) { throw 'unexpected caller' }
}
throw 'Stata caller not found'
