version 15.0
clear all
set more off

* x/sx and w have disjoint support. z/sz = x/sx + gap*w holds exactly.
* Replicating paired shocks makes the specified means satisfy the score
* equations. The weak direction has material effects, but its score
* variance under robust inference is of order gap^2. With IID inference
* the gap is larger. In both cases the original slope covariance supports
* two independent Stata restrictions. The weights balance x/sx and w under
* each model's Hessian. A PPML offset makes the deepest positive case's
* true predictor exactly zero while the weak slope remains nonzero.
set obs 132
generate byte cell = mod(floor((_n - 1)/2), 4)
generate byte shock = cond(mod(_n, 2), -1, 1)
generate double a = cond(cell < 2, 2*cell - 1, 0)
generate double w = cond(cell >= 2, 2*cell - 5, 0)
generate byte included = _n <= 128
generate double ols_fw = cond(w == 0, 1, 2)
generate double ppml_fw = .
generate double mean_ols = 3 + a/2 + w/4
generate double mean_ppml = .
generate double eta_offset = -w/4
generate double y_ols = .
generate double y_ppml = .
generate double x = .
generate double z = .

mata:
void pair_oracle(real scalar poisson, real scalar robust, real scalar offset)
{
    real matrix design, bread, variance, change
    real rowvector b
    real colvector q, fitted, residual
    real scalar n, correction

    design = st_data(., ("a", "w"), "included"), J(128, 1, 1)
    if (poisson) {
        q = st_data(., "ppml_fw", "included")
        fitted = st_data(., "mean_ppml", "included")
        residual = st_data(., "y_ppml", "included") - fitted
        if (offset) b = (0, 0.25, 0)
        else b = (0, ln(2), ln(4))
        correction = 1
    }
    else {
        q = st_data(., "ols_fw", "included")
        fitted = J(128, 1, 1)
        residual = st_data(., ("y_ols", "mean_ols"), "included") * (1 \ -1)
        b = (0.5, 0.25, 3)
        correction = 3
    }
    n = sum(q)
    bread = cholinv(quadcross(design, q:*fitted, design))
    if (robust) {
        variance = n/(n-correction) * bread *
            quadcross(design, q:*residual:^2, design) * bread
    }
    else variance = sum(q:*residual:^2)/(n-correction) * bread
    change = (1/st_numscalar("sx"), -1/(st_numscalar("sx")*st_numscalar("gap")), 0 \
              0, 1/(st_numscalar("sz")*st_numscalar("gap")), 0 \
              0, 0, 1)
    st_matrix("stable_b", b)
    st_matrix("stable_V", variance)
    st_matrix("expected_b", b*change')
    st_matrix("expected_V", change*variance*change')
    st_numscalar("expected_wald", (b[1]^2/variance[1,1] + b[2]^2/variance[2,2]) /
        (poisson ? 1 : 2))
    st_numscalar("expected_N", n)
}
end

foreach units in same different {
    scalar sx = cond("`units'" == "same", 1, 2^6)
    scalar sz = cond("`units'" == "same", 1, 2^-6)
    quietly replace x = sx*a
    local common_x = scalar(sx)
    local common_z = scalar(sz)
    foreach model in ols_iid ols_robust ppml_robust ols_weak ppml_weak ppml_adverse {
        local poisson = substr("`model'", 1, 4) == "ppml"
        local robust = "`model'" != "ols_iid"
        local weak = inlist("`model'", "ols_weak", "ppml_weak", "ppml_adverse")
        local offset = "`model'" == "ppml_weak"
        local adverse = "`model'" == "ppml_adverse"
        local vce = cond(`robust', "robust", "iid")
        scalar gap = cond(`weak', 2^-40, cond(`robust', 2^-18, 2^-14))
        quietly replace z = sz*(a + gap*w)
        assert (z/sz - x/sx)/gap == w
        quietly replace y_ols = mean_ols + shock*cond(!`robust' | w == 0, 0.5, gap) if included
        quietly replace mean_ppml = cond(`offset', 1, 4*2^w)
        quietly replace ppml_fw = cond(`offset', ols_fw, cond(w == -1, 4, 1))
        local strong_shock = cond(`offset', 0.5, 1)
        quietly replace y_ppml = mean_ppml + shock*cond(w == 0, `strong_shock', gap) if included
        local weak_z : display %21.16g (sz*gap)
        mata: pair_oracle(`poisson', `robust', `offset')
        if `poisson' {
            local adjustment = cond(`offset', "offset(eta_offset)", "")
            local estimate "quietly"
            if `adverse' {
                * A nontrivial mean can magnify Hessian rounding into inaccurate
                * original slope/intercept cross-covariances at this gap.
                matrix previous_b = e(b)
                matrix previous_V = e(V)
                local estimate "capture noisily"
            }
            `estimate' feppml y_ppml x z [fw=ppml_fw] in 1/130, ///
                vce(robust) threads(1) eta(saved_fit) mu(saved_mean) ///
                residuals(saved_residual) `adjustment'
            local rc = _rc
            if `rc' {
                assert `adverse' & `rc' == 430
                mata: assert(st_matrix("e(b)") == st_matrix("previous_b"))
                mata: assert(st_matrix("e(V)") == st_matrix("previous_V"))
                assert e(sample) == included
                foreach output in saved_fit saved_mean saved_residual {
                    confirm new variable `output'
                }
                continue
            }
            assert e(separated) == 0 & e(N_physical) == 128
            assert e(df_resid) == expected_N - 3
            assert abs(saved_fit-ln(mean_ppml)) <= 1e-9 + 1e-7*abs(ln(mean_ppml)) if included
            assert abs(saved_mean-mean_ppml) <= 1e-9 + 1e-7*mean_ppml if included
            assert missing(saved_mean) if !included
            quietly predict double predicted_fit, xb
            quietly predict double predicted_mean, mu
            assert abs(predicted_fit-ln(mean_ppml)) <= 1e-9 + 1e-7*abs(ln(mean_ppml)) if included
            assert abs(predicted_mean-mean_ppml) <= 1e-9 + 1e-7*mean_ppml if included
            assert abs(saved_residual-(y_ppml-mean_ppml)) <= 1e-9 + 1e-7*abs(y_ppml-mean_ppml) if included
            assert missing(predicted_mean) if !included
            drop saved_mean predicted_mean
        }
        else {
            quietly fereg y_ols x z [fw=ols_fw] in 1/130, ///
                vce(`vce') threads(1) residuals(saved_residual)
            assert e(df_r) == expected_N - 3
            quietly predict double saved_fit if e(sample), xbd
            quietly predict double predicted_fit if e(sample), xb
            assert abs(saved_fit-mean_ols) <= 1e-9 + 1e-7*mean_ols if included
            assert abs(predicted_fit-mean_ols) <= 1e-9 + 1e-7*mean_ols if included
            assert abs(saved_residual-(y_ols-mean_ols)) <= 1e-9 + 1e-7*abs(y_ols-mean_ols) if included
        }
        assert missing(saved_fit) & missing(saved_residual) if !included
        assert missing(predicted_fit) if !included
        assert e(sample) == included
        assert e(N) == expected_N
        assert e(df_m) == 2
        matrix actual_b = e(b)
        matrix actual_V = e(V)
        local names : colnames actual_b
        assert "`names'" == "x z _cons"
        forvalues i = 1/3 {
            assert abs(el(actual_b,1,`i')-el(expected_b,1,`i')) <= 1e-10 + 1e-7*abs(el(expected_b,1,`i'))
            local term : word `i' of `names'
            assert abs(_se[`term']-sqrt(el(expected_V,`i',`i'))) <= 1e-10 + 1e-6*sqrt(el(expected_V,`i',`i'))
            forvalues j = 1/3 {
                assert abs(el(actual_V,`i',`j')-el(expected_V,`i',`j')) <= 1e-10 + 1e-6*abs(el(expected_V,`i',`j'))
            }
        }
        quietly lincom `common_x'*x + `common_z'*z
        assert abs(r(estimate)-el(stable_b,1,1)) <= 1e-10 + 1e-7*abs(el(stable_b,1,1))
        assert abs(r(se)-sqrt(el(stable_V,1,1))) <= 1e-10 + 1e-6*sqrt(el(stable_V,1,1))
        quietly lincom `weak_z'*z
        assert abs(r(estimate)-el(stable_b,1,2)) <= 1e-10 + 1e-7*abs(el(stable_b,1,2))
        assert abs(r(se)-sqrt(el(stable_V,2,2))) <= 1e-10 + 1e-6*sqrt(el(stable_V,2,2))
        quietly test (x=0) (z=0)
        assert r(df) == 2
        if `poisson' {
            assert abs(r(chi2)-expected_wald) <= 1e-10 + 1e-6*expected_wald
            assert abs(e(chi2)-expected_wald) <= 1e-10 + 1e-6*expected_wald
        }
        else {
            assert abs(r(F)-expected_wald) <= 1e-10 + 1e-6*expected_wald
            assert abs(e(F)-expected_wald) <= 1e-10 + 1e-6*expected_wald
        }
        drop saved_fit predicted_fit saved_residual
    }
}

* An exact multiple keeps the earlier slot. A much weaker direction with
* order-one residuals yields an unrepresentable original covariance. Preserve
* the established diagnostic and the previous fit on that refusal.
quietly replace x = a
quietly replace mean_ppml = 4*2^w
quietly replace y_ols = 3 + a/2 + shock/2 if included
quietly replace y_ppml = 4*2^a + shock if included
foreach fit in fereg feppml {
    local poisson = "`fit'" == "feppml"
    local depvar = cond(`poisson', "y_ppml", "y_ols")
    local option = cond(`poisson', "mu(refused)", "residuals(refused)")
    quietly replace z = 2*x
    quietly `fit' `depvar' x z in 1/130, vce(robust) threads(1)
    matrix alias_b = e(b)
    matrix alias_V = e(V)
    local names : colnames alias_b
    assert "`names'" == "x o.z _cons"
    assert e(df_m) == 1 & _b[z] == 0 & _se[z] == 0
    assert e(sample) == included
    if `poisson' quietly replace y_ppml = mean_ppml + shock if included
    else quietly replace y_ols = mean_ols + shock/2 if included
    quietly replace z = x + 2^-41*w
    capture noisily `fit' `depvar' x z in 1/130, vce(robust) threads(1) `option'
    assert _rc == 430
    mata: assert(st_matrix("e(b)") == st_matrix("alias_b"))
    mata: assert(st_matrix("e(V)") == st_matrix("alias_V"))
    assert e(sample) == included
    capture confirm new variable refused
    assert _rc == 0
}

display as result "END_FIXTURE|slope_pair_recovery|rc=0"
