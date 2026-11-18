version 18.0
clear
set more off

set obs 10
generate long obs_id = _n
generate byte a = real(word("1 2 2 2 2 2 2 2 2 3", _n))
generate byte b = real(word("1 1 1 1 1 2 2 2 2 3", _n))
generate byte f = real(word("1 2 2 3 3 2 2 3 3 1", _n))
generate byte z = real(word("1 1 2 1 2 1 2 1 2 2", _n))
generate double x = real(word("-1.5 -1.0 -0.4 0.2 0.9 -0.8 -0.1 0.5 1.2 1.7", _n))
generate double y = 1.4*x + 0.7*(f == 3) - 0.3*(z == 2) + ///
    0.2*(f == 3)*(z == 2) + 0.1*a - 0.2*b

generate int ab = 10*a+b
generate int cluster = mod(_n,3)+1
tempfile source
save `source'

// The sealed reghdfe receipt witnesses its historical replacement-base
// parameterization. SPEC retains our original axis; compare identified
// contrasts, fitted values and their covariance with independent dummy OLS.
foreach model in factor interaction {
    foreach inference in iid robust cluster {
        use `source', clear
        local rhs c.x i.f
        local contrasts 2
        local tolerance 5e-9
        if "`model'" == "interaction" {
            local rhs c.x i.f##i.z
            local contrasts 4
            local tolerance 5e-8
        }
        local vce
        local reference_vce
        if "`inference'" == "robust" local vce vce(robust)
        if "`inference'" == "robust" local reference_vce vce(robust)
        if "`inference'" == "cluster" local vce vce(cluster) cluster(cluster)
        if "`inference'" == "cluster" local reference_vce vce(cluster cluster)
        fvexpand `rhs'
        local original_names `"`r(varlist)'"'
        local original_count : word count `original_names'
        local original_count = `original_count'+1
        _fereg_estimate_ols_test y `rhs', absorb(a#b) residual(rebase_resid) `vce'
        assert e(N) == 8
        assert e(N_full) == 10
        assert e(num_singletons) == 2
        assert e(fereg_receipt)[1,5] == `original_count'
        assert `"`e(fereg_result_wire)'"' == "1.7"
        assert e(fereg_status)[2,2] == 2
        local names : colfullnames e(b)
        assert strpos(`"`names'"', "1b.f") > 0
        assert strpos(`"`names'"', "2b.f") == 0
        assert e(sample) == inrange(obs_id, 2, 9)
        assert missing(rebase_resid) if !e(sample)
        matrix b = e(b)
        matrix V = e(V)
        matrix status = e(fereg_status)
        generate byte sample = e(sample)
        forvalues j=1/`original_count' {
            if status[`j',2] == 1 | status[`j',2] == 5 continue
            assert b[1,`j'] == 0
            forvalues k=1/`original_count' {
                assert V[`j',`k'] == 0
                assert V[`k',`j'] == 0
            }
        }
        matrix L = J(`contrasts', `original_count', 0)
        forvalues j=1/`=`original_count'-1' {
            local name : word `j' of `original_names'
            if "`name'" == "x" matrix L[1,`j'] = 1
            if "`name'" == "2.f" matrix L[2,`j'] = -1
            if "`name'" == "3.f" matrix L[2,`j'] = 1
            if `contrasts' == 4 {
                matrix L[3,`j'] = L[2,`j']
                if "`name'" == "2.f#2.z" matrix L[3,`j'] = -1
                if "`name'" == "3.f#2.z" matrix L[3,`j'] = 1
                if "`name'" == "2.z" | "`name'" == "2.f#2.z" matrix L[4,`j'] = 1
            }
        }
        matrix contrast_b = L*b'
        matrix contrast_V = L*V*L'
        quietly regress y `rhs' i.ab if sample, `reference_vce'
        predict double reference_residual if sample, residuals
        assert abs(rebase_resid-reference_residual) <= `tolerance' if sample
        local expression1 x
        local expression2 3.f
        local expression3 3.f + 3.f#2.z
        local expression4 2.z
        matrix reference_b = J(`contrasts', 1, 0)
        matrix reference_variance = J(`contrasts', 1, 0)
        forvalues j=1/`contrasts' {
            quietly lincom `expression`j''
            matrix reference_b[`j',1] = r(estimate)
            matrix reference_variance[`j',1] = r(se)^2
            assert abs(contrast_b[`j',1]-r(estimate)) <= `tolerance'*max(abs(r(estimate)),1)
        }
        forvalues j=1/`contrasts' {
            forvalues k=1/`contrasts' {
                quietly lincom (`expression`j'') + (`expression`k'')
                local reference_covariance = (r(se)^2-reference_variance[`j',1]-reference_variance[`k',1])/2
                assert abs(contrast_V[`j',`k']-`reference_covariance') <= ///
                    `tolerance'*max(abs(`reference_covariance'),1)
            }
        }
    }
}

// Saved effects select a different preparation owner. Preserve original slots,
// status/rank/sample and science for the same one-FE model across output modes.
foreach inference in iid robust cluster {
    foreach memory in fast low {
        use `source', clear
        local vce
        local reference_vce
        if "`inference'" == "robust" local reference_vce vce(robust)
        if "`inference'" == "cluster" local reference_vce vce(cluster cluster)
        if "`inference'" == "robust" local vce vce(robust)
        if "`inference'" == "cluster" local vce vce(cluster) cluster(cluster)
        _fereg_estimate_ols_test y c.x i.f, absorb(a) `vce' memory(`memory') threads(1)
        matrix ordinary_b = e(b)
        matrix ordinary_V = e(V)
        matrix ordinary_status = e(fereg_status)
        local ordinary_rank = e(rank)
        generate byte ordinary_sample = e(sample)
        local original_names : colfullnames e(b)
        assert `"`original_names'"' == "x 1b.f 2.f 3o.f _cons"
        _fereg_estimate_ols_test y c.x i.f, absorb(alpha=a) `vce' memory(`memory') threads(4) ///
            residual(saved_residual) xbd(saved_fitted)
        assert e(sample) == ordinary_sample
        assert e(rank) == `ordinary_rank'
        assert mreldif(e(fereg_status),ordinary_status) == 0
        assert mreldif(e(b),ordinary_b) <= 5e-9
        assert mreldif(e(V),ordinary_V) <= 5e-9
        local saved_names : colfullnames e(b)
        assert `"`saved_names'"' == `"`original_names'"'
        quietly regress y c.x i.f i.a if ordinary_sample, `reference_vce'
        predict double reference_residual if ordinary_sample, residuals
        assert abs(saved_residual-reference_residual) <= 5e-9 if ordinary_sample
        assert abs(saved_fitted-(y-reference_residual)) <= 5e-9 if ordinary_sample
    }
}

// The singleton groups contain every row of level 1. Its explicit base
// coordinate survives their removal. Other bases keep their original slots.
foreach specification in ib1.f ib(first).f ib2.f ib3.f ib(last).f fvset {
    local rhs `specification'
    local base 1
    if "`rhs'" == "ib2.f" local base 2
    if inlist("`rhs'", "ib3.f", "ib(last).f", "fvset") local base 3
    if "`rhs'" == "fvset" {
        fvset base 3 f
        local rhs i.f
    }
    quietly fereg y x `rhs', absorb(a#b) residuals(explicit_r)
    assert e(sample) == inrange(obs_id,2,9)
    local names : colnames e(b)
    local base_name : word `=`base'+1' of `names'
    assert "`base_name'" == "`base'b.f" & e(fereg_status)[`base'+1,2] == 2
    scalar bx = _b[x]
    scalar sex = _se[x]
    quietly regress y x `rhs' i.ab if inrange(obs_id,2,9)
    predict double ref_r if e(sample), residuals
    assert abs(ref_r-explicit_r) <= 5e-9 if e(sample)
    assert abs(bx-_b[x]) <= 5e-9 & abs(sex-_se[x]) <= 5e-9
    drop explicit_r ref_r
    fvset clear f
}

capture noisily fereg y x, absorb(a#b)
assert _rc == 0
assert `"`e(cmd)'"' == "fereg"
assert e(N) == 8
display "END_FIXTURE|estimate_ols_categorical_final_sample_rebase|rc=0"
