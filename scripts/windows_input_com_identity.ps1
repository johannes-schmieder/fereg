# Called only by the new automation Stata, before any test starts. Bind the
# cleanup handle to that caller rather than guessing from a process-name list.
param([Parameter(Mandatory=$true)][string]$Output)
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class FeregTemporaryPath {
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode)]
    private static extern uint GetTempPath2W(uint size, StringBuilder path);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode)]
    private static extern uint GetTempPathW(uint size, StringBuilder path);
    public static string Read() {
        var path = new StringBuilder(32768);
        uint length;
        try { length = GetTempPath2W((uint)path.Capacity, path); }
        catch (EntryPointNotFoundException) { length = GetTempPathW((uint)path.Capacity, path); }
        if (length == 0 || length >= path.Capacity) throw new InvalidOperationException();
        return path.ToString();
    }
}
'@
$process = Get-CimInstance Win32_Process -Filter "ProcessId=$PID"
for ($depth = 0; $depth -lt 6; $depth++) {
    $process = Get-CimInstance Win32_Process -Filter ("ProcessId=" + $process.ParentProcessId)
    if ($null -eq $process) { throw 'missing caller' }
    if ($process.ExecutablePath -ieq 'C:\Program Files\Stata19\StataMP-64.exe') {
        $identity = [ordered]@{
            pid = [int]$process.ProcessId
            ticks = (Get-Process -Id $process.ProcessId).StartTime.ToUniversalTime().Ticks
            session = [int]$process.SessionId
            # Match Rust 1.85.1, including SYSTEM's GetTempPath2 directory.
            temporary = [FeregTemporaryPath]::Read()
        }
        $identity | ConvertTo-Json -Compress | Set-Content -LiteralPath $Output -Encoding ASCII
        exit 0
    }
    if ($process.Name -notin @('cmd.exe', 'powershell.exe')) { throw 'unexpected caller' }
}
throw 'Stata caller not found'
