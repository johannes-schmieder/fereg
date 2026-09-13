version 18
args expected_rc
clear all
set more off
set obs 4096
generate long row=_n
generate byte g=1+mod(row,4)
generate double z=sin(row*.129)+cos(row*.247)
generate double w=1+g
generate double y=10+2*g+3*z+sin(row*.371)
quietly regress y ibn.g ibn.g#c.z [aw=w], noconstant
matrix prior_b=e(b)
matrix prior_v=e(V)
generate byte prior_sample=e(sample)
predict double oracle_residual, residuals
capture noisily fereg y [aw=w], noconstant absorb(g##c.z) solver(lsmr) memory(fast) threads(4) residuals(native_e)
local actual_rc=_rc
display "GENERIC_CAP_ACTUAL_RC=`actual_rc' EXPECTED=`expected_rc'"
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
    display "GENERIC_CAP_MODEL=" %21.0f runtime[25,2]
    display "GENERIC_CAP_PEAK=" %21.0f runtime[13,2]
}
display "GENERIC_CAP_PASS rc=0"
exit 0
