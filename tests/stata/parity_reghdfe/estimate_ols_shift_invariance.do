version 18.0
clear all
set more off
if c(MP) set processors 1
set obs 800

* Each of the 100 crossed FE cells contains the entire three-sign cube.
* Its signs are mutually orthogonal and orthogonal to every FE dummy.
* Adding A*g to x changes neither the exact model space nor its solution:
* beta=(2,-3), residual=w, rank=2, FE rank=19, RSS=800, residual df=779.
generate byte g = 1 + floor((_n-1)/80)
generate byte h = 1 + mod(floor((_n-1)/8),10)
generate byte sign_id = mod(_n-1,8)
generate double u = 2*mod(sign_id,2)-1
generate double v = 2*mod(floor(sign_id/2),2)-1
generate double w = 2*mod(floor(sign_id/4),2)-1
generate double y = 5*g + 7*h + 2*u - 3*v + w
generate double x = u
generate double aw = 1 + mod(g+2*h,7)
generate double aw2 = aw^2
quietly summarize aw, meanonly
scalar raw_weight_sum = r(sum)
quietly summarize aw2, meanonly
scalar robust_variance = 800/779*r(sum)/(raw_weight_sum^2)

foreach shift in 1e4 1e8 {
    replace x = `shift'*g + u
    foreach weight_kind in unit analytic {
        local weight ""
        if "`weight_kind'" == "analytic" local weight "[aw=aw]"
        foreach mode in fast low {
            foreach solver in auto lsmr cmg {
                quietly fereg y x v `weight', absorb(g h) solver(`solver') ///
                    memory(`mode') poolsize(1) threads(1) residuals(native_residual)
                assert e(N) == 800 & e(sample) == 1
                assert e(rank) == 2 & e(df_m) == 2
                assert e(df_a) == 19 & e(df_r) == 779
                assert e(converged) == 1 & e(num_singletons) == 0
                assert abs(_b[x]-2) <= 1e-8
                assert abs(_b[v]+3) <= 1e-8
                assert abs(e(rss)-800) <= 1e-5
                assert abs(_se[x]-1/sqrt(779)) <= 1e-9
                assert abs(_se[v]-1/sqrt(779)) <= 1e-9
                assert abs(native_residual-w) <= 1e-6
                assert e(fereg_status)[1,2] == 1
                assert e(fereg_status)[2,2] == 1
                forvalues certificate = 1/3 {
                    assert e(fereg_certificate)[`certificate',1] == 1
                    assert e(fereg_certificate)[`certificate',2] == 1
                }
                assert e(fereg_certificate)[7,2] <= 5e-10
                assert e(fereg_certificate)[24,2] == 1
                assert e(fereg_certificate)[32,2] == 1
                assert "`e(fereg_memory_selected)'" == "`mode'"
                assert e(fereg_runtime)[9,2] >= 0
                assert e(fereg_runtime)[13,2] > 0
                assert e(fereg_memory)[1,1] == 1
                assert e(fereg_memory)[1,2] == 2
                assert inlist(e(fereg_memory)[2,2],1,2)
                if "`mode'" == "low" assert e(fereg_runtime)[9,2] > 0
                local requested = cond("`solver'"=="auto",1,cond("`solver'"=="lsmr",3,7))
                assert e(fereg_solver)[1,1] == `requested'
                drop native_residual
            }
        }
    }
}

* For analytic weights constant inside each sign cube, HC1 slope variance is
* (N/df_r)*sum(raw_w^2)/sum(raw_w)^2. This checks the repaired score design
* used by inference independently of the IID inverse cross-product.
foreach mode in fast low {
    quietly fereg y x v [aw=aw], absorb(g h) memory(`mode') poolsize(1) ///
        threads(1) vce(robust) residuals(robust_residual)
    assert abs(_b[x]-2) <= 1e-8 & abs(_b[v]+3) <= 1e-8
    assert abs(_se[x]-sqrt(robust_variance)) <= 1e-9
    assert abs(_se[v]-sqrt(robust_variance)) <= 1e-9
    assert abs(e(V)[1,2]) <= 1e-10
    assert e(df_r) == 779
    assert abs(robust_residual-w) <= 1e-6
    assert e(fereg_certificate)[24,2] == 1
    assert e(fereg_certificate)[32,2] == 1
    drop robust_residual
}

* The original-scale products contain large cancelling terms. Check their
* reconstruction in the small outcome units, without a tolerance proportional
* to the large nuisance contribution. Requested FE channels remain normalized.
quietly fereg y x v, absorb(saved_g=g saved_h=h) memory(fast) threads(1) ///
    residuals(saved_residual) xb(saved_xb) xbd(saved_xbd) d(saved_d)
assert abs(_b[x]-2) <= 1e-8 & abs(_b[v]+3) <= 1e-8
assert abs(y-saved_xbd-saved_residual) <= 2e-6
assert abs(y-saved_xb-saved_g-saved_h-saved_residual) <= 2e-6
assert abs(saved_d-saved_g-saved_h) <= 2e-6
assert abs(saved_residual-w) <= 1e-6
quietly summarize saved_g, meanonly
assert abs(r(mean)) <= 2e-6
quietly summarize saved_h, meanonly
assert abs(r(mean)) <= 2e-6
predict double predicted_residual, residuals
predict double predicted_xbd, xbd
assert abs(predicted_residual-saved_residual) <= 1e-10
assert abs(predicted_xbd-saved_xbd) <= 2e-6
drop saved_g saved_h saved_residual saved_xb saved_xbd saved_d ///
    predicted_residual predicted_xbd

* Exact FE-only columns retain their omission semantics; repairing the
* identified cases must not promote numerical projection noise to rank.
generate double exactly_absorbed = 1e8*g
quietly fereg y exactly_absorbed v, absorb(g h) threads(1) residuals(omitted_residual)
assert e(rank) == 1 & e(df_r) == 780
assert _b[exactly_absorbed] == 0 & _se[exactly_absorbed] == 0
assert abs(_b[v]+3) <= 1e-8
assert abs(e(rss)-4000) <= 1e-5
assert abs(omitted_residual-2*u-w) <= 1e-6
drop exactly_absorbed omitted_residual

* A finite iteration budget is a resource contract, not an omission policy.
* Establish success first, then require atomic refusal of the same valid
* weighted model when its explicit iterative solver receives only one step.
quietly fereg y x v [aw=aw], absorb(g h) solver(lsmr) tolerance(1e-12) threads(1)
assert e(rank) == 2 & e(fereg_solver)[1,3] > 1
quietly regress y u v if g <= 4
generate byte prior_sample = e(sample)
tempname prior_b prior_V
matrix `prior_b' = e(b)
matrix `prior_V' = e(V)
local prior_N = e(N)
local prior_df_r = e(df_r)
foreach mode in fast low {
    capture noisily fereg y x v [aw=aw], absorb(failed_g=g failed_h=h) ///
        solver(lsmr) tolerance(1e-12) iterate(1) memory(`mode') poolsize(1) ///
        threads(1) residuals(failed_residual) xb(failed_xb) xbd(failed_xbd) d(failed_d)
    local refusal_rc = _rc
    assert `refusal_rc' == 430
    foreach output in failed_g failed_h failed_residual failed_xb failed_xbd failed_d {
        confirm new variable `output'
    }
    assert "`e(cmd)'" == "regress"
    assert e(N) == `prior_N' & e(df_r) == `prior_df_r'
    assert mreldif(e(b),`prior_b') == 0
    assert mreldif(e(V),`prior_V') == 0
    assert e(sample) == prior_sample
}

display as result "END_FIXTURE|estimate_ols_shift_invariance|rc=0"
