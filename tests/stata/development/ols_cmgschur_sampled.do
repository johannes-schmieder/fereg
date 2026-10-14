version 18.0
clear all
set more off

* Degree-30 workers and lower-degree workers share one weighted graph.
* Every distinct worker-firm cell contains two observations.
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

* Independent dense dummy regression checks the unchanged target model.
quietly regress y x1 x2 i.worker i.firm [aw=weight], vce(robust)
matrix reference_b = e(b)
matrix reference_v = e(V)
scalar reference_df_r = e(df_r)
predict double reference_residual, residuals

* Omitted and explicit zero sampling retain the same unsampled science.
quietly fereg y x1 x2 [aw=weight], absorb(worker firm) solver(cmgschur) ///
    vce(robust) threads(1) tolerance(1e-10) residuals(unsampled)
matrix unsampled_b = e(b)
matrix unsampled_v = e(V)
matrix unsampled_solver = e(fereg_solver)
quietly fereg y x1 x2 [aw=weight], absorb(worker firm) solver(cmgschur) ///
    cmgsamples(0) vce(robust) threads(1) tolerance(1e-10) residuals(zero)
assert mreldif(e(b), unsampled_b) == 0
assert mreldif(e(V), unsampled_v) == 0
assert zero == unsampled
assert e(fereg_cmg_samples) == 0
assert e(fereg_cmg_seed) == 0
forvalues row = 1/2 {
    foreach column in 1 2 3 4 5 6 7 8 11 12 13 14 15 16 {
        assert e(fereg_solver)[`row',`column'] == unsampled_solver[`row',`column']
    }
}
drop unsampled zero

foreach mode in fast low {
    foreach threads in 1 4 {
        foreach seed in 0 1 9007199254740991 {
            forvalues repetition = 1/2 {
                local seed_option "cmgseed(`seed')"
                if `seed' == 0 & `repetition' == 1 local seed_option
                quietly fereg y x1 x2 [aw=weight], absorb(sw=worker sf=firm) ///
                    solver(cmgschur) cmgsamples(2) `seed_option' ///
                    memory(`mode') poolsize(1) threads(`threads') ///
                    vce(robust) tolerance(1e-10) residuals(resid)
                assert e(fereg_solver)[1,1] == 8
                assert e(fereg_solver)[1,2] == 8
                assert e(fereg_cmg_degree) == 5
                assert e(fereg_cmg_samples) == 2
                assert e(fereg_cmg_seed) == `seed'
                assert e(df_r) == reference_df_r
                assert e(sample) == 1
                forvalues j = 1/2 {
                    assert abs(e(b)[1,`j'] - reference_b[1,`j']) <= 1e-10
                    forvalues k = 1/2 {
                        assert abs(e(V)[`j',`k'] - reference_v[`j',`k']) <= 1e-10
                    }
                }
                predict double xb, xb
                predict double d, d
                assert abs(resid-reference_residual) <= 1e-9
                assert abs(y-xb-d-resid) <= 1e-10
                assert abs(d-sw-sf) <= 1e-10
                if `repetition' == 1 {
                    matrix repeat_b = e(b)
                    matrix repeat_v = e(V)
                    matrix repeat_solver = e(fereg_solver)
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
                    forvalues row = 1/2 {
                        foreach column in 1 2 3 4 5 6 7 8 11 12 13 14 15 16 {
                            assert e(fereg_solver)[`row',`column'] == repeat_solver[`row',`column']
                        }
                    }
                    drop repeat_resid repeat_sw repeat_sf
                }
                drop sw sf resid xb d
            }
        }
    }
}

matrix prior_b = e(b)
matrix prior_v = e(V)
scalar prior_seed = e(fereg_cmg_seed)
generate byte prior_sample = e(sample)

* Invalid public and hidden requests cannot publish output or replace e().
foreach command in fereg _fereg_estimate_ols {
    * Both integer maxima reach native topology validation without rounding.
    * The unsupported one-FE model prevents allocating billions of trees.
    capture noisily `command' y x1, absorb(worker) solver(cmgschur) ///
        cmgsamples(4294967295) cmgseed(9007199254740991) resid(rejected)
    assert _rc == 498
    assert mreldif(e(b), prior_b) == 0
    foreach options in "cmgsamples(-1)" "cmgsamples(1.5)" ///
        "cmgsamples(4294967296)" "cmgsamples(NaN)" "cmgseed(0)" ///
        "cmgsamples(0) cmgseed(1)" "cmgsamples(2) cmgseed(-1)" ///
        "cmgsamples(2) cmgseed(9007199254740992)" ///
        "cmgsamples(2) cmgseed(9007199254740990.5)" {
        capture noisily `command' y x1, absorb(sw=worker sf=firm) ///
            solver(cmgschur) `options' resid(rejected)
        assert _rc == 198
        assert mreldif(e(b), prior_b) == 0
        assert mreldif(e(V), prior_v) == 0
        assert e(fereg_cmg_seed) == prior_seed
        assert e(sample) == prior_sample
    }
    foreach solver in auto cmg lsmr map it squarem {
        foreach options in "cmgsamples(0)" "cmgsamples(1)" "cmgseed(0)" {
            capture noisily `command' y x1, absorb(worker firm) ///
                solver(`solver') `options' resid(rejected)
            assert _rc == 198
            assert mreldif(e(b), prior_b) == 0
            assert e(fereg_cmg_seed) == prior_seed
        }
    }
}
foreach options in "cmgsamples(0)" "cmgsamples(1)" "cmgseed(0)" {
    capture noisily feppml y x1, absorb(worker firm) `options'
    assert _rc == 198
    assert mreldif(e(b), prior_b) == 0
    assert e(fereg_cmg_seed) == prior_seed
}

* A sampled solve exhausted after one step preserves the previous result.
assert e(fereg_solver)[1,3] > 1
foreach mode in fast low {
    capture noisily fereg y x1 x2 [aw=weight], absorb(sw=worker sf=firm) ///
        solver(cmgschur) cmgsamples(2) cmgseed(9007199254740991) ///
        memory(`mode') poolsize(1) threads(4) vce(robust) ///
        tolerance(1e-10) iterate(1) residuals(rejected)
    assert _rc == 430
    assert mreldif(e(b), prior_b) == 0
    assert mreldif(e(V), prior_v) == 0
    assert e(fereg_cmg_seed) == prior_seed
    assert e(sample) == prior_sample
}
foreach output in sw sf rejected {
    confirm new variable `output'
}

display as result "END_FIXTURE|ols_cmgschur_sampled|rc=0"
