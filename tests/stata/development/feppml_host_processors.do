version 18.0
clear all
set more off

local original_processors = c(processors)
local parallel_processors = min(4, c(processors_lic), c(processors_mach))
set obs 4096
generate long obs_id = _n
generate int g = 1 + mod(_n-1, 32)
generate double x = sin(_n/17) + mod(_n, 7)/5
generate double y = 1 + mod(_n, 19)
tempname b_ref V_ref b_prior V_prior

* Host settings and native requests are independent, including threads(0).
local host_values 1
if `parallel_processors' > 1 local host_values 1 `parallel_processors'
foreach host of local host_values {
    set processors `host'
    foreach threads in 1 4 0 {
        foreach memory in fast low {
            feppml y x, absorb(saved_g=g) threads(`threads') memory(`memory') ///
                separation(none) mu(mu_fit) eta(eta_fit) xb(xb_fit) d(d_fit)
            assert c(processors) == `host'
            assert e(converged) == 1
            assert e(feppml_requested_threads) == `threads'
            if `threads' > 0 assert e(feppml_threads) == `threads'
            else assert e(feppml_threads) >= 1
            assert abs(eta_fit-xb_fit-d_fit) <= 1e-10
            assert abs(saved_g-d_fit) <= 1e-10
            if `host' == 1 & `threads' == 1 & "`memory'" == "fast" {
                matrix `b_ref' = e(b)
                matrix `V_ref' = e(V)
                generate double mu_ref = mu_fit
            }
            assert mreldif(e(b), `b_ref') <= 1e-10
            assert mreldif(e(V), `V_ref') <= 1e-10
            assert abs(mu_fit-mu_ref) <= 1e-10*(1+abs(mu_ref))
            drop saved_g mu_fit eta_fit xb_fit d_fit
        }
    }
    foreach prior in populated null {
        quietly regress y x if obs_id <= 4000
        matrix `b_prior' = e(b)
        matrix `V_prior' = e(V)
        generate byte prior_sample = e(sample)
        if "`prior'" == "null" ereturn clear
        foreach stage in panic allocation postplugin {
            capture noisily _fereg_estimate_ppml y x, absorb(failed_g=g) ///
                separation(none) threads(4) mu(failed_mu) inject(`stage')
            local native_rc = _rc
            assert `native_rc' == cond("`stage'"=="panic",3499,cond("`stage'"=="allocation",909,3498))
            assert c(processors) == `host'
            confirm new variable failed_g
            confirm new variable failed_mu
            if "`prior'" == "populated" {
                assert "`e(cmd)'" == "regress"
                assert mreldif(e(b), `b_prior') == 0
                assert mreldif(e(V), `V_prior') == 0
                assert e(sample) == prior_sample
            }
            else {
                assert "`e(cmd)'" == ""
                assert missing(e(N))
            }
        }
        drop prior_sample
    }
}
set processors `original_processors'
display as result "END_FIXTURE|feppml_host_processors|rc=0"
