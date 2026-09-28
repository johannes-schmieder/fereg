version 18.0
clear all
set more off

local original_processors = c(processors)
local parallel_processors = min(4, c(processors_lic), c(processors_mach))
local host_values 1
if `parallel_processors' > 1 local host_values 1 `parallel_processors'
set obs 4096
generate long obs_id = _n
generate int g = 1 + mod(_n-1, 32)
generate double x = sin(_n/17) + mod(_n, 7)/5
generate double y = 2 + x/3 + mod(_n, 19)
generate double w = 1 + mod(_n, 3)
tempname b_ref V_ref b_prior V_prior

* An independent explicit-dummy fit checks the weighted clustered result.
quietly regress y x i.g [aw=w], vce(cluster g)
scalar oracle_b = _b[x]
scalar oracle_se = _se[x]
foreach host of local host_values {
    set processors `host'
    foreach threads in 1 4 0 {
        foreach memory in fast low {
            quietly fereg y x [aw=w], absorb(saved_g=g) threads(`threads') ///
                memory(`memory') residuals(resid_fit) vce(cluster g) dof(none)
            assert c(processors) == `host'
            assert e(fereg_requested_threads) == `threads'
            if `threads' > 0 assert e(fereg_threads) == `threads'
            else assert e(fereg_threads) >= 1
            assert abs(_b[x]-oracle_b) <= 1e-10
            // dof(none) changes the HC1 finite-sample multiplier. The existing
            // scientific fixtures independently check those inference rules.
            assert !missing(_se[x]) & _se[x] > 0
            assert abs(y-_b[x]*x-_b[_cons]-saved_g-resid_fit) < 1e-9
            if `host' == 1 & `threads' == 1 & "`memory'" == "fast" {
                matrix `b_ref' = e(b)
                matrix `V_ref' = e(V)
                generate double residual_ref = resid_fit
            }
            assert mreldif(e(b), `b_ref') < 1e-10
            assert mreldif(e(V), `V_ref') < 1e-10
            assert abs(resid_fit-residual_ref) < 1e-9
            drop saved_g resid_fit
        }
    }
    quietly fereg, version
    assert c(processors) == `host'
    assert r(requested_threads) == 0
    foreach prior in populated null {
        quietly regress y x if obs_id <= 4000
        matrix `b_prior' = e(b)
        matrix `V_prior' = e(V)
        if "`prior'" == "null" ereturn clear
        foreach failure in syntax sample {
            if "`failure'" == "syntax" capture noisily fereg y x, unsupported_scope_option
            else capture noisily fereg y x if 0, absorb(failed_g=g) residuals(failed_resid)
            local fit_rc = _rc
            if "`failure'" == "syntax" assert `fit_rc' == 198
            else assert `fit_rc' != 0
            assert c(processors) == `host'
            confirm new variable failed_g
            confirm new variable failed_resid
            if "`prior'" == "populated" {
                assert "`e(cmd)'" == "regress"
                assert mreldif(e(b), `b_prior') == 0
                assert mreldif(e(V), `V_prior') == 0
                assert e(sample) == (obs_id <= 4000)
            }
            else assert "`e(cmd)'" == ""
        }
    }
}

program drop _fereg_estimate_public
program define _fereg_estimate_public, eclass
    assert c(processors) == 1
    assert c(noisily) == $FEREG_SCOPE_EXPECT_NOISILY
    _fereg_estimate_ols_test `0'
end
foreach host of local host_values {
    set processors `host'
    foreach prior in populated null {
        quietly regress y x if obs_id <= 4000
        matrix `b_prior' = e(b)
        matrix `V_prior' = e(V)
        if "`prior'" == "null" ereturn clear
        foreach prefix in noisily quietly {
            global FEREG_SCOPE_EXPECT_NOISILY = ("`prefix'"=="noisily")
            foreach stage in posthold postplugin poststripe postereturn postscalars postdiagnostics precommit {
                capture noisily `prefix' fereg y x, absorb(failed_g=g) ///
                    residual(failed_resid) threads(4) inject(`stage')
                assert _rc == 3498
                assert c(processors) == `host'
                confirm new variable failed_g
                confirm new variable failed_resid
                if "`prior'" == "populated" {
                    assert "`e(cmd)'" == "regress"
                    assert mreldif(e(b), `b_prior') == 0
                    assert mreldif(e(V), `V_prior') == 0
                    assert e(sample) == (obs_id <= 4000)
                }
                else assert "`e(cmd)'" == ""
            }
        }
    }
}
macro drop FEREG_SCOPE_EXPECT_NOISILY
program drop _fereg_estimate_public
set processors `original_processors'
display as result "END_FIXTURE|ols_public_scope|rc=0"
