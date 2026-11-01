version 18.0
* Second Stata/MP process of the Windows parallel-input qualification, started
* by scripts/windows_input_qualification.ps1 with the solver trace enabled.
* It uses the package that windows_ci_runtime.do installed and hash-verified,
* plus the hidden test adapters in stata/, and writes the marker file only after
* the oracle finished without error.
args marker
clear all
set more off
if c(os) != "Windows" exit 9
local root = subinstr(`"`c(pwd)'"', char(92), "/", .)
capture erase `"`marker'"'
sysdir set PLUS `"`root'/target/windows-plus"'
sysdir set PERSONAL `"`root'/target/windows-personal"'
global S_ADO "UPDATES;BASE;PLUS;."
adopath ++ `"`root'/stata"'
findfile fereg_windows.plugin
assert strpos(subinstr(`"`r(fn)'"', char(92), "/", .), "/target/windows-plus/") > 0
do "tests/stata/development/parallel_input_oracle.do"
tempname handle
file open `handle' using `"`marker'"', write replace text
file write `handle' "WINDOWS_INPUT_ORACLE=PASS" _n
file close `handle'
