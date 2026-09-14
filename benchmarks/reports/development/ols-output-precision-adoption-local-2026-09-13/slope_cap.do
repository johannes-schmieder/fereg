version 18
args expected_rc input_path
clear all
set more off
set linesize 255
use "`input_path'", clear
generate long original_row=_n
quietly regress y x2
matrix prior_b=e(b)
matrix prior_v=e(V)
generate byte prior_sample=e(sample)
capture noisily fereg y x1-x16 [aw=w], absorb(saved_g=g1 g2#c.x1) threads(4) solver(auto) poolsize(10) vce(robust) residuals(saved_e) memory(fast)
local actual_rc=_rc
display "SLOPE_CAP_ACTUAL_RC=`actual_rc' EXPECTED=`expected_rc'"
assert original_row==_n
if `actual_rc'==909 {
    assert mreldif(e(b),prior_b)==0
    assert mreldif(e(V),prior_v)==0
    assert e(sample)==prior_sample
    foreach v in saved_e saved_g {
        capture confirm variable `v'
        assert _rc==111
    }
    display "SLOPE_CAP_ROLLBACK_PASS"
}
else if `actual_rc'==0 {
    assert e(N)==30720
    assert e(rank)==15
    assert e(df_r)==20209
    assert e(sample)==prior_sample
    matrix runtime=e(fereg_runtime)
    matrix solver=e(fereg_solver)
    display "SLOPE_CAP_PEAK=" %21.0f runtime[13,2]
    display "SLOPE_CAP_MODEL=" %21.0f runtime[25,2]
    display "SLOPE_CAP_FINAL_SOLVER=" solver[2,2]
    display "SLOPE_CAP_FINAL_FALLBACK=" solver[2,11]
    display "SLOPE_CAP_FINAL_WARNINGS=" solver[2,12]
    export delimited original_row saved_e saved_g using "outputs.csv", replace
}
else assert 0
if `expected_rc'>=0 assert `actual_rc'==`expected_rc'
display "SLOPE_CAP_PASS rc=0"
exit 0
