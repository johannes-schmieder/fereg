version 18
args expected_rc
clear all
set more off
set obs 4096
generate long row=_n
generate byte g=1+mod(row,4)
generate double y=10+2*g+sin(row*.371)
quietly regress y ibn.g, noconstant
matrix prior_b=e(b)
matrix prior_v=e(V)
generate byte prior_sample=e(sample)
predict double oracle_residual, residuals
capture noisily fereg y, noconstant absorb(g) solver(cg) memory(fast) threads(1) residuals(native_e)
local actual_rc=_rc
display "CG_CAP_ACTUAL_RC=`actual_rc' EXPECTED=`expected_rc'"
assert `actual_rc'==`expected_rc'
assert row==_n
assert e(sample)==prior_sample
if `expected_rc'==909 {
    assert mreldif(e(b),prior_b)==0
    assert mreldif(e(V),prior_v)==0
    capture confirm variable native_e
    assert _rc==111
}
else {
    assert abs(native_e-oracle_residual)<1e-9
    matrix runtime=e(fereg_runtime)
    assert runtime[25,2]==212992
    display "CG_CAP_MODEL=" runtime[25,2]
    display "CG_CAP_PEAK=" runtime[13,2]
}
display "CG_CAP_PASS rc=0"
exit 0
