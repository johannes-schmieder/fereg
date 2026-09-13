clear all
set more off
adopath ++ "<isolated-bench>/baseline/stata"
adopath ++ "<isolated-bench>/baseline/dist"
use "<isolated-bench>/preparation-data/pw_partial-n245760-s104729.dta", clear
capture noisily fereg y x1 x2 x3 x4 x5 x6 x7 x8 x9 x10 x11 x12 x13 x14 x15 x16 x17 x18 x19 x20 x21 x22 x23 x24 x25 x26 x27 x28 x29 x30 x31 x32 x33 x34 x35 x36 x37 x38 x39 x40 x41 x42 x43 x44 x45 x46 x47 x48 x49 x50  [pw=w], absorb(saved_g=g1 g2) threads(16) solver(auto) poolsize(10) vce(cluster c1 c2) residuals(saved_e)
local result_rc=_rc
display "DIAGNOSTIC_PW_RC|" `result_rc'
exit, STATA clear
