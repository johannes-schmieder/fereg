version 15.0
clear all
set more off
quietly set obs 120
generate long id = _n
generate byte g = mod(_n,6)
generate double x = cos(_n/7)
generate double y = g + sin(_n/3)

* The finite predictor of an empty coefficient axis is zero, including the
* accepted report-only no-FE noconstant model. No estimation semantics change.
foreach model in grouped ungrouped slopeonly {
    local absorb_option
    local constant_option noconstant
    if "`model'" == "grouped" local absorb_option "absorb(g)"
    if "`model'" == "slopeonly" {
        local absorb_option "absorb(g#c.x)"
        local constant_option
        quietly regress y ibn.g#c.x if id > 5 in 1/110, noconstant
        predict double oracle_fit if e(sample), xb
    }
    quietly fereg y if id > 5 in 1/110, `absorb_option' `constant_option' residuals(res)
    assert `"`e(cmd)'"' == "fereg"
    assert el(e(fereg_receipt),1,5) == 0
    capture confirm matrix e(b)
    assert _rc == 111
    tempvar sample
    generate byte `sample' = e(sample)
    foreach storage in double float long byte {
        predict `storage' xb if id > 15 in 1/100, xb
        assert xb == 0 if `sample' & id > 15 & id <= 100
        assert missing(xb) if !`sample' | id <= 15 | id > 100
        drop xb
    }
    predict double default_xb
    predict double d, d
    predict double xbd, xbd
    predict double response, residuals
    assert default_xb == 0 if `sample'
    assert d == xbd & xbd == y-res & response == res if `sample'
    if "`model'" == "slopeonly" {
        assert reldif(xbd, oracle_fit) < 1e-10 if `sample'
        drop oracle_fit
    }
    assert missing(default_xb) & missing(d) & missing(xbd) & missing(response) if !`sample'
    assert e(sample) == `sample'
    capture confirm matrix e(b)
    assert _rc == 111
    drop res default_xb d xbd response `sample'
}

* Empty-axis xb does not require saved residuals; d still does.
quietly fereg y, absorb(g) noconstant
predict double xb
assert xb == 0
capture predict double unavailable_d, d
assert _rc == 498
confirm new variable unavailable_d

di as result "END_FIXTURE|ols_empty_axis_prediction|rc=0"
