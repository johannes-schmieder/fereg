version 18.0
clear all
set more off

if c(os) != "Windows" {
    exit 9
}
local root = subinstr(`"`c(pwd)'"', char(92), "/", .)
capture erase "dist/fereg_windows.plugin.sha256"
capture erase "target/windows-plugin.verified"
capture erase "target/windows-installed.verified"

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

* Exercise the same net-install boundary as users, before loading the plugin.
shell python scripts/stage_local_install.py --platform windows --output target/windows-install
confirm file "target/windows-install/fereg.pkg"
capture mkdir "target/windows-plus"
capture mkdir "target/windows-personal"
sysdir set PLUS `"`root'/target/windows-plus"'
sysdir set PERSONAL `"`root'/target/windows-personal"'
global S_ADO "UPDATES;BASE;PLUS;."
net install fereg, from(`"`root'/target/windows-install"') replace
findfile fereg.ado
assert strpos(subinstr(`"`r(fn)'"', char(92), "/", .), "/target/windows-plus/") > 0
findfile fereg_windows.plugin
local installed_plugin = subinstr(`"`r(fn)'"', char(92), "/", .)
confirm file "`installed_plugin'"
* shell forwards its command string: use OS quotes, not Stata compound quotes.
shell powershell.exe -NoLogo -NoProfile -NonInteractive ///
    -ExecutionPolicy Bypass -File "scripts/verify_windows_plugin.ps1" ///
    -Plugin "`installed_plugin'" -Checksum "`root'/dist/fereg_windows.plugin.sha256" ///
    -Receipt "`root'/target/windows-installed.verified"
confirm file "target/windows-installed.verified"
file open `verification' using "target/windows-installed.verified", read text
file read `verification' installed_verified
file close `verification'
assert `"`installed_verified'"' == `"`verified'"'
shell powershell.exe -NoLogo -NoProfile -NonInteractive ///
    -ExecutionPolicy Bypass -File "scripts/windows_ci_report.ps1" -Action start -Stage stata_smoke
* Diagnostic r(9101) identifies this unchanged fixture; it cannot confer PASS.
capture noisily do "tests/stata/installation/windows_ci_smoke.do"
if _rc exit 9101
shell powershell.exe -NoLogo -NoProfile -NonInteractive ///
    -ExecutionPolicy Bypass -File "scripts/windows_ci_report.ps1" -Action start -Stage single_runtime
* Diagnostic r(9102) identifies this unchanged fixture; it cannot confer PASS.
capture noisily do "tests/stata/installation/single_runtime.do"
if _rc exit 9102
* Diagnostic r(9103) identifies this unchanged fixture; it cannot confer PASS.
capture noisily do "tests/stata/development/ols_solver_reporting.do"
if _rc exit 9103
* Diagnostic r(9104) identifies this unchanged fixture; it cannot confer PASS.
capture noisily do "tests/stata/development/ols_cmg_raw_residual_oracle.do"
if _rc exit 9104
* Diagnostic r(9105) identifies this unchanged fixture; it cannot confer PASS.
capture noisily do "tests/stata/development/ols_mixed_solver_reports.do"
if _rc exit 9105
* Diagnostic r(9106) identifies this unchanged fixture; it cannot confer PASS.
capture noisily do "tests/stata/parity_ppmlhdfe/feppml_one_fe_dummy_oracle.do"
if _rc exit 9106
* Exercise the migrated within paths with independent dummy-model witnesses.
* Diagnostic r(9107) identifies this unchanged fixture; it cannot confer PASS.
capture noisily do "tests/stata/development/within_weighted_ols.do"
if _rc exit 9107
* Diagnostic r(9108) identifies this unchanged fixture; it cannot confer PASS.
capture noisily do "tests/stata/development/ols_three_fe_batch_oracle.do"
if _rc exit 9108
* Diagnostic r(9109) identifies this unchanged fixture; it cannot confer PASS.
capture noisily do "tests/stata/development/lsmr_batch_oracle.do"
if _rc exit 9109
* This fixture locates its exact input through the source ado path. The installed
* binary has already been hash-verified; the source and installed ado bytes match.
adopath ++ "`root'/stata"
* Diagnostic r(9110) identifies this unchanged fixture; it cannot confer PASS.
capture noisily do "tests/stata/development/within_plugin_slope_boundary.do"
if _rc exit 9110
* Replay, bootstrap:/jackknife: and the displayed header on Windows.
* Diagnostic r(9111) identifies this fixture; it cannot confer PASS.
capture noisily do "tests/stata/postestimation/replay_and_prefixes.do"
if _rc exit 9111
shell powershell.exe -NoLogo -NoProfile -NonInteractive ///
    -ExecutionPolicy Bypass -File "scripts/windows_ci_report.ps1" -Action pass
