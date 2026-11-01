version 17.0
clear all
set more off

* Failures after parallel loading began: a worker panic and an SDK read
* failure inside the first phase. Each must fail the transaction (never retry
* serially), roll back every result and output, close the numeric window, and
* leave the session able to load in parallel again with exact results.
local workers = min(4, c(processors_mach), c(processors_lic))
assert `workers' >= 2
set obs 400003
generate long row = _n
generate double x1 = sin(_n * .37)
generate float x2 = mod(_n, 97) / 7
generate int x3 = mod(_n * 7, 30011) - 15000
generate double y = x1 + .1 * x2 + 1e-5 * x3 + sin(_n * .011)
generate double py = exp(.2 * x1 + .01 * x2) * (1 + mod(_n, 3))
generate long g = mod(_n * 31, 5003)
generate byte keep = mod(_n, 5) != 2

quietly regress y x1 if row <= 20
tempname prior_b prior_V serial_b serial_V
matrix `prior_b' = e(b)
matrix `prior_V' = e(V)
local prior_cmd `"`e(cmd)'"'
generate byte __prior_sample = e(sample)

foreach stage in input_worker_panic input_read_failure {
    foreach sample in "" "if keep" {
        capture noisily _fereg_estimate_ols_test y x1 x2 x3 `sample', absorb(g) ///
            threads(`workers') residual(__failed_resid) inject(`stage')
        local failed_rc = _rc
        assert `failed_rc' != 0
        confirm new variable __failed_resid
        assert `"`e(cmd)'"' == `"`prior_cmd'"'
        assert mreldif(e(b), `prior_b') == 0
        assert mreldif(e(V), `prior_V') == 0
        assert e(sample) == __prior_sample
        di as result "FAILURE_RC|ols|`stage'|`sample'|`failed_rc'"

        capture noisily _fereg_estimate_ppml py x1 x2 `sample', absorb(g) ///
            threads(`workers') mu(__failed_mu) inject(`stage')
        local failed_rc = _rc
        assert `failed_rc' != 0
        confirm new variable __failed_mu
        assert `"`e(cmd)'"' == `"`prior_cmd'"'
        assert mreldif(e(b), `prior_b') == 0
        di as result "FAILURE_RC|ppml|`stage'|`sample'|`failed_rc'"

        * The next action opens a fresh window and is exact at equal workers.
        quietly _fereg_estimate_ols_test y x1 x2 x3 `sample', absorb(g) ///
            threads(`workers') inject(serial_input)
        matrix `serial_b' = e(b)
        matrix `serial_V' = e(V)
        quietly _fereg_estimate_ols_test y x1 x2 x3 `sample', absorb(g) ///
            threads(`workers')
        assert mreldif(e(b), `serial_b') == 0
        assert mreldif(e(V), `serial_V') == 0
        quietly regress y x1 if row <= 20
    }
}

display as result "END_FIXTURE|parallel_input_failures|rc=0"
