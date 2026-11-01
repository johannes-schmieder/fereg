version 18.0
* Second Stata/MP process of the Windows parallel-input qualification, started
* by scripts/windows_input_qualification.ps1 with the solver trace enabled.
* It uses the package that windows_ci_runtime.do installed and hash-verified,
* plus the hidden test adapters in stata/. The marker file records PASS, or the
* failing stage (1 setup, 2 oracle) and its return code; nothing else leaves.
args marker
clear all
set more off
if c(os) != "Windows" exit 9
local root = subinstr(`"`c(pwd)'"', char(92), "/", .)
capture erase `"`marker'"'
capture noisily {
    sysdir set PLUS `"`root'/target/windows-plus"'
    sysdir set PERSONAL `"`root'/target/windows-personal"'
    global S_ADO "UPDATES;BASE;PLUS;."
    adopath ++ `"`root'/stata"'
    findfile fereg_windows.plugin
    assert strpos(subinstr(`"`r(fn)'"', char(92), "/", .), "/target/windows-plus/") > 0
}
local rc = _rc
local stage 1
if !`rc' {
    capture noisily do "tests/stata/development/parallel_input_oracle.do"
    local rc = _rc
    local stage 2
}
tempname handle
file open `handle' using `"`marker'"', write replace text
if `rc' file write `handle' "WINDOWS_INPUT_ORACLE=FAIL|stage=`stage'|rc=`rc'" _n
else file write `handle' "WINDOWS_INPUT_ORACLE=PASS" _n
file close `handle'
