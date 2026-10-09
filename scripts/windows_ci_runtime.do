version 18.0
clear all
set more off

if c(os) != "Windows" {
    exit 9
}
local root = subinstr(`"`c(pwd)'"', char(92), "/", .)
capture erase "dist/fereg_windows.plugin.sha256"
capture erase "target/windows-plugin.verified"

shell powershell.exe -NoLogo -NoProfile -NonInteractive ///
    -ExecutionPolicy Bypass -File "scripts/build_windows.ps1"
confirm file "dist/fereg_windows.plugin"
confirm file "dist/fereg_windows.plugin.sha256"

shell powershell.exe -NoLogo -NoProfile -NonInteractive ///
    -ExecutionPolicy Bypass -File "scripts/windows_ci_report.ps1" -Action start -Stage checksum
shell powershell.exe -NoLogo -NoProfile -NonInteractive ///
    -ExecutionPolicy Bypass -File "scripts/verify_windows_plugin.ps1" ///
    -Plugin "dist/fereg_windows.plugin" ///
    -Checksum "dist/fereg_windows.plugin.sha256" ///
    -Receipt "target/windows-plugin.verified"
confirm file "target/windows-plugin.verified"
tempname verification
file open `verification' using "target/windows-plugin.verified", read text
file read `verification' verified
file close `verification'
assert regexm(`"`verified'"', "^WINDOWS_PLUGIN_SHA256=[0-9a-f]{64}$")

adopath ++ `"`root'/stata"'
adopath ++ `"`root'/dist"'
shell powershell.exe -NoLogo -NoProfile -NonInteractive ///
    -ExecutionPolicy Bypass -File "scripts/windows_ci_report.ps1" -Action start -Stage stata_smoke
do "tests/stata/installation/windows_ci_smoke.do"
shell powershell.exe -NoLogo -NoProfile -NonInteractive ///
    -ExecutionPolicy Bypass -File "scripts/windows_ci_report.ps1" -Action start -Stage single_runtime
do "tests/stata/installation/single_runtime.do"
shell powershell.exe -NoLogo -NoProfile -NonInteractive ///
    -ExecutionPolicy Bypass -File "scripts/windows_ci_report.ps1" -Action pass
