version 18.0
clear all
set more off

local original_processors = c(processors)
local parallel_processors = min(4, c(processors_lic), c(processors_mach))
local host_values 1
if `parallel_processors' > 1 local host_values 1 `parallel_processors'
set obs 80
generate long obs_id = _n
generate double x = mod(_n, 11) / 5 - 1
generate byte fe = 1 + mod(_n-1, 8)
generate double y = 0.5 + mod(7*_n, 13)
generate double invalid_y = cond(_n==1, -1, y)
tempname prior_b prior_V

* Exercise real public syntax and native domain errors before replacing the
* syntax adapter solely to reach private injection boundaries below.
foreach host of local host_values {
    set processors `host'
    foreach prior in populated null {
        quietly regress y x if obs_id <= 60
        matrix `prior_b' = e(b)
        matrix `prior_V' = e(V)
        if "`prior'" == "null" ereturn clear
        foreach failure in syntax domain {
            if "`failure'" == "syntax" capture noisily feppml y x, unsupported_scope_option
            else capture noisily feppml invalid_y x, absorb(failed_fe=fe) mu(failed_mu)
            local fit_rc = _rc
            assert `fit_rc' == cond("`failure'"=="syntax", 198, 459)
            assert c(processors) == `host'
            confirm new variable failed_fe
            confirm new variable failed_mu
            if "`prior'" == "populated" {
                assert "`e(cmd)'" == "regress"
                assert mreldif(e(b), `prior_b') == 0
                assert mreldif(e(V), `prior_V') == 0
                assert e(sample) == (obs_id <= 60)
            }
            else assert "`e(cmd)'" == ""
        }
    }
}

program drop _fereg_ppml
program define _fereg_ppml, eclass
    assert c(processors) == 1
    assert c(noisily) == $FEREG_SCOPE_EXPECT_NOISILY
    _fereg_estimate_ppml `0'
end

foreach host of local host_values {
    set processors `host'
    foreach prior in populated null {
        quietly regress y x if obs_id <= 60
        matrix `prior_b' = e(b)
        matrix `prior_V' = e(V)
        if "`prior'" == "null" ereturn clear
        foreach prefix in noisily quietly {
            global FEREG_SCOPE_EXPECT_NOISILY = ("`prefix'"=="noisily")
            foreach stage in panic allocation postplugin precommit {
                capture noisily `prefix' feppml y x, absorb(failed_fe=fe) ///
                    mu(failed_mu) threads(4) inject(`stage')
                local fit_rc = _rc
                assert `fit_rc' == cond("`stage'"=="panic",3499,cond("`stage'"=="allocation",909,3498))
                assert c(processors) == `host'
                confirm new variable failed_fe
                confirm new variable failed_mu
                if "`prior'" == "populated" {
                    assert "`e(cmd)'" == "regress"
                    assert mreldif(e(b), `prior_b') == 0
                    assert mreldif(e(V), `prior_V') == 0
                    assert e(sample) == (obs_id <= 60)
                }
                else assert "`e(cmd)'" == ""
            }
        }
    }
}
macro drop FEREG_SCOPE_EXPECT_NOISILY
program drop _fereg_ppml
quietly feppml y x, absorb(saved_fe=fe) mu(saved_mu) threads(4)
assert e(converged) == 1
assert e(feppml_threads) == 4
assert c(processors) == `parallel_processors'
confirm variable saved_fe saved_mu
set processors `original_processors'
display as result "END_FIXTURE|feppml_public_scope|rc=0"
