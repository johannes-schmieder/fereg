version 18.0
clear all
set more off
args source
adopath ++ "`source'/stata"
set obs 2000
generate double y = _n
generate str12 label = "numeric frame"
quietly _fereg_memory_baseline
assert r(used) >= 2000 * 20 & r(used) < .
assert r(allocated) >= r(used) & r(allocated) < .
scalar single_allocated = r(allocated)
quietly regress y
estimates store held
frame create auxiliary
frame auxiliary: quietly set obs 4000
frame auxiliary: quietly generate strL text = "long string storage"
quietly _fereg_memory_baseline
assert missing(r(used))
assert r(allocated) >= single_allocated & r(allocated) < .
assert e(N) == 2000
assert c(frame) == "default"
frame drop auxiliary
quietly _fereg_memory_baseline
assert r(used) < . & r(allocated) >= r(used)
estimates restore held
assert e(N) == 2000
matrix report = J(11,2,0)
matrix report[1,1] = 1
matrix report[1,2] = 1
matrix report[2,1] = 1
matrix report[2,2] = 1
matrix report[11,1] = 1
_fereg_memory_validate report
matrix report[8,1] = 1
capture _fereg_memory_validate report
assert _rc == 459
matrix report[8,1] = 0
matrix report[4,2] = 123
capture _fereg_memory_validate report
assert _rc == 459
display "FEREG_MEMORY_BASELINE_PASS"
exit, clear
