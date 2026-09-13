clear all
set more off
set processors 4
adopath ++ "<isolated-bench>/candidate5-final/stata"
adopath ++ "<isolated-bench>/candidate5-final/dist"
use "<isolated-bench>/qr-public-data/near_unit_iid-n16384-s31415927.dta", clear
quietly regress y x1
matrix before_b=e(b)
capture noisily fereg y x1-x16, absorb(g1) threads(4) solver(auto) poolsize(10)
local fit_rc=_rc
display "QR5_PUBLIC_DIAGNOSTIC_RC=" `fit_rc'
assert `fit_rc'==498
matrix after_b=e(b)
assert mreldif(before_b,after_b)==0
display "QR5_PUBLIC_DIAGNOSTIC_COMPLETE rollback=1"
exit, STATA clear
