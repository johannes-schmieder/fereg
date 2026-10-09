version 18.0
clear all
set more off

capture erase "windows-ci.status"
shell powershell.exe -NoLogo -NoProfile -NonInteractive ///
    -ExecutionPolicy Bypass -File "scripts/windows_ci_report.ps1" -Action init
capture noisily do "scripts/windows_ci_runtime.do"
local result = _rc
if `result' {
    shell powershell.exe -NoLogo -NoProfile -NonInteractive ///
        -ExecutionPolicy Bypass -File "scripts/windows_ci_report.ps1" ///
        -Action fail -StataReturnCode `result'
}

* Always terminate the controller's wait, including an early Stata failure.
* Diagnostics never grant success: the fixed controller validates them too.
shell powershell.exe -NoLogo -NoProfile -NonInteractive ///
    -ExecutionPolicy Bypass -File "scripts/windows_ci_report.ps1" ///
    -Action status -StataReturnCode `result'
exit `result'
