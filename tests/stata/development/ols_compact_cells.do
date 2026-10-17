version 18.0
clear all
set more off

* Repeated cells, non-movers, degree-30 workers and positive varying weights.
set obs 180
generate long worker = _n
generate byte degree = cond(mod(worker, 3) == 0, 30, 1 + mod(worker, 6))
expand 2 * degree
bysort worker: generate byte period = _n
generate int firm = 1 + mod(worker * 13 + floor((period - 1) / 2) * 7, 61)
generate double weight = 1 + mod(_n, 7) / 3
generate double x1 = sin(_n * .31)
generate double x2 = cos(_n * .17)
generate double y = .7*x1 - .2*x2 + sin(worker*.11) + cos(firm*.3) + ///
    (mod(_n*37, 101) - 50)*.001
quietly regress y x1 x2 i.worker i.firm [aw=weight], vce(robust)
matrix reference_b = e(b)
matrix reference_v = e(V)
scalar reference_df_r = e(df_r)
predict double reference_residual, residuals

foreach arm in cells_v2 cells_v2_df cells_deferred cells_m cells_m_lanes cells_s {
    foreach mode in fast low {
        foreach threads in 1 4 {
            forvalues repetition = 1/2 {
                quietly _fereg_estimate_ols_test y x1 x2, aweight(weight) ///
                    absorb(sw=worker sf=firm) solver(auto) inject(routing_`arm') ///
                    memory(`mode') poolsize(1) threads(`threads') ///
                    vce(robust) tolerance(1e-10) residual(resid)
                assert e(df_r) == reference_df_r
                assert e(sample) == 1
                forvalues j = 1/2 {
                    assert abs(e(b)[1,`j'] - reference_b[1,`j']) <= 1e-10
                    forvalues k = 1/2 {
                        assert abs(e(V)[`j',`k'] - reference_v[`j',`k']) <= 1e-10
                    }
                }
                predict double finite, xb
                assert abs(resid-reference_residual) <= 1e-9
                assert abs(y-finite-sw-sf-resid) <= 1e-9
                if `repetition' == 1 {
                    matrix repeat_b = e(b)
                    matrix repeat_v = e(V)
                    generate double repeat_resid = resid
                    generate double repeat_sw = sw
                    generate double repeat_sf = sf
                }
                else {
                    assert mreldif(e(b), repeat_b) == 0
                    assert mreldif(e(V), repeat_v) == 0
                    assert resid == repeat_resid
                    assert sw == repeat_sw
                    assert sf == repeat_sf
                    drop repeat_resid repeat_sw repeat_sf
                }
                drop resid sw sf finite
            }
        }
    }
}
* A failed private request preserves the prior result and all staged columns.
quietly regress y x1 x2
matrix prior_b = e(b)
generate byte prior_sample = e(sample)
foreach arm in cells_v2 cells_v2_df cells_deferred cells_m cells_m_lanes cells_s {
    capture noisily _fereg_estimate_ols_test y x1 x2, absorb(sw=worker sf=firm) ///
        solver(lsmr) inject(routing_`arm') residual(failed)
    assert _rc == 198
    assert mreldif(e(b), prior_b) == 0
    assert e(sample) == prior_sample
    confirm new variable failed
    confirm new variable sw
    confirm new variable sf
}
di as result "COMPACT_CELLS_STATA_PASS"
di as result "END_FIXTURE|ols_compact_cells|rc=0"
