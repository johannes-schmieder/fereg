version 18.0
clear all
set more off

* The private controller supplies a fresh, hash-verified source archive.
if c(os) != "Windows" {
    exit 9
}
local root = subinstr(`"`c(pwd)'"', char(92), "/", .)
capture erase "windows-ci.status"
capture erase "dist/fereg_windows.plugin.sha256"
capture erase "target/windows-plugin.verified"

* Stata shell return codes alone do not prove a native build succeeded.
* The build writes its checksum only after the PE audit and real loader pass.
shell powershell.exe -NoLogo -NoProfile -NonInteractive ///
    -ExecutionPolicy Bypass -File "scripts/build_windows.ps1"
confirm file "dist/fereg_windows.plugin"
confirm file "dist/fereg_windows.plugin.sha256"
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
do "tests/stata/installation/windows_ci_smoke.do"
do "tests/stata/installation/single_runtime.do"

tempname status
file open `status' using `"`root'/windows-ci.status"', write text replace
file write `status' "WINDOWS_CI=PASS" _n
file close `status'
display as result "WINDOWS_CI=PASS"
