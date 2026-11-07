param([int[]]$Before = @())
# Read-only, categorical activation diagnosis. No names, command lines,
# captions, registry identifiers or process identifiers leave this helper.
$ErrorActionPreference = 'Stop'
$class = (Get-Item -LiteralPath 'Registry::HKEY_CLASSES_ROOT\stata.StataOLEApp\CLSID').GetValue('')
$classKey = Get-Item -LiteralPath ('Registry::HKEY_CLASSES_ROOT\CLSID\' + $class)
$server = [string](Get-Item -LiteralPath ($classKey.PSPath + '\LocalServer32')).GetValue('')
$arguments = $server -replace '^"?C:\\Program Files\\Stata19\\StataMP-64\.exe"?\s*', ''
$argumentKind = if (-not $arguments) { 0 } elseif ($arguments -match '^[-/]Automation$') { 1 } elseif ($arguments -match '^[-/]Embedding$') { 2 } else { 3 }
$runAsKind = 0
$appId = [string]$classKey.GetValue('AppID')
if ($appId) {
    $appKey = Get-Item -LiteralPath ('Registry::HKEY_LOCAL_MACHINE\SOFTWARE\Classes\AppID\' + $appId)
    $runAs = [string]$appKey.GetValue('RunAs')
    $runAsKind = if (-not $runAs) { 0 } elseif ($runAs -ieq 'Interactive User') { 1 } else { 2 }
    if ($appKey.GetValue('LocalService')) { $runAsKind = 3 }
}
$new = @(Get-Process -Name StataMP-64 -ErrorAction SilentlyContinue |
    Where-Object { $_.Id -notin $Before -and $_.Path -ieq 'C:\Program Files\Stata19\StataMP-64.exe' })
$processKind = [Math]::Min($new.Count, 2)
if ($new.Count -gt 0) {
    Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class FeregWindowState {
    private delegate bool Callback(IntPtr window, IntPtr parameter);
    [DllImport("user32.dll")] private static extern bool EnumWindows(Callback callback, IntPtr parameter);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern int GetClassNameW(IntPtr window, StringBuilder name, int length);
    [DllImport("user32.dll")] private static extern bool IsWindowEnabled(IntPtr window);
    public static bool Modal(int process) {
        bool modal = false;
        EnumWindows(delegate(IntPtr window, IntPtr parameter) {
            uint owner;
            GetWindowThreadProcessId(window, out owner);
            if (owner == process) {
                var name = new StringBuilder(256);
                GetClassNameW(window, name, name.Capacity);
                modal |= name.ToString() == "#32770" || !IsWindowEnabled(window);
            }
            return true;
        }, IntPtr.Zero);
        return modal;
    }
}
'@
    foreach ($process in $new) {
        if ([FeregWindowState]::Modal($process.Id)) { $processKind += 2; break }
    }
}
# 83RAP: R=RunAs/LocalService; A=arguments; P=0 absent, 1/2 one/multiple,
# 3/4 one/multiple with a dialog or disabled top-level window.
83000 + 100 * $runAsKind + 10 * $argumentKind + $processKind
