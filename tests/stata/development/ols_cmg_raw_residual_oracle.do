version 18
clear all
set more off
set obs 800
generate long row = _n - 1
generate long worker = floor(row/8)
generate long firm = mod(worker + mod(row,8),10)
generate double x = mod(row*37+11,997)/97 - 5
generate double y = .7*x + sin(worker*.13) + cos(firm*.29) + (mod(row*53+7,101)-50)*.001

quietly regress y x i.worker i.firm
scalar oracle_b = _b[x]
predict double oracle_residual, residuals

foreach memory in fast low {
    foreach solver in auto lsmr cmg {
        quietly fereg y x, absorb(worker firm) solver(`solver') memory(`memory') poolsize(1) residuals(resid)
        assert abs(_b[x] - oracle_b) <= 2e-8
        assert abs(resid - oracle_residual) <= 2e-8
        drop resid
    }
}

* Public CG rejection must precede both variable and estimation publication.
matrix before_b = e(b)
matrix before_v = e(V)
local before_cmd `"`e(cmd)'"'
foreach estimator in fereg feppml {
    capture noisily `estimator' y x, absorb(worker firm) solver(cg) residuals(rejected_residual)
    assert _rc == 198
    assert `"`e(cmd)'"' == `"`before_cmd'"'
    assert mreldif(e(b),before_b) == 0
    assert mreldif(e(V),before_v) == 0
    capture confirm variable rejected_residual
    assert _rc == 111
}
display as result "END_FIXTURE|ols_cmg_raw_residual_oracle|rc=0"
