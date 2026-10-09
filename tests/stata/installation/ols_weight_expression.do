version 18.0
clear all
set more off
set seed 290103
set obs 300

generate long row = _n
generate int firm = ceil(_n / 10)
generate double scale = 0.5 + mod(_n, 7) / 4
generate double x = rnormal()
generate double y = 1 + 2*x + sin(firm) + rnormal() * scale
generate double inverse_scale = 1/scale
generate double inverse_scale_sq = 1/scale^2

* Expression weights equal the same weights stored in a variable, exactly.
foreach kind in aw pw fw {
    local expression "1/scale"
    local stored inverse_scale
    if "`kind'" == "fw" {
        local expression "1 + mod(row, 3)"
        generate long stored_fw = 1 + mod(row, 3)
        local stored stored_fw
    }
    quietly fereg y x [`kind'=`stored'], absorb(firm) vce(robust) threads(1)
    matrix stored_b = e(b)
    matrix stored_V = e(V)
    local stored_N = e(N)
    assert `"`e(wexp)'"' == "= `stored'"
    quietly fereg y x [`kind'=`expression'], absorb(firm) vce(robust) threads(1)
    assert mreldif(e(b), stored_b) == 0
    assert mreldif(e(V), stored_V) == 0
    assert e(N) == `stored_N'
    assert `"`e(wtype)'"' == cond("`kind'"=="aw","aweight",cond("`kind'"=="pw","pweight","fweight"))
    assert `"`e(wexp)'"' == "= `expression'"
    if "`kind'" == "fw" drop stored_fw
}

* The expression is evaluated only on if/in candidates; an excluded row whose
* expression is undefined cannot fail the estimate, and the sample matches.
replace scale = 0 in 1
quietly fereg y x [aw=inverse_scale_sq] if row > 1, absorb(firm) threads(1)
matrix stored_b = e(b)
generate byte stored_sample = e(sample)
quietly fereg y x [aw=1/scale^2] if row > 1, absorb(firm) threads(1)
assert mreldif(e(b), stored_b) == 0
assert e(sample) == stored_sample
assert `"`e(wexp)'"' == "= 1/scale^2"

* No temporary weight variable survives the call.
quietly describe, varlist
assert `"`r(varlist)'"' == "row firm scale x y inverse_scale inverse_scale_sq stored_sample"

display as result "END_FIXTURE|ols_weight_expression|rc=0"
