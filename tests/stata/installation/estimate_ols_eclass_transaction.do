version 18.0
clear all
set more off

quietly findfile stfixest.ado
local ado_path = subinstr(`"`r(fn)'"', "\", "/", .)
local suffix "/stata/stfixest.ado"
assert substr(`"`ado_path'"', -strlen(`"`suffix'"'), .) == `"`suffix'"'
local root = substr(`"`ado_path'"', 1, strlen(`"`ado_path'"') - strlen(`"`suffix'"'))
import delimited using `"`root'/tests/fixtures/ols/ols_core.csv"', clear

* Hold a populated, unrelated eclass result and prove exact named-state
* restoration at every deterministic hidden transaction boundary.
quietly regress y c.x1 if obs_id <= 20
generate byte __stfx_prior_sample = e(sample)
local prior_scalars : e(scalars)
local prior_macros : e(macros)
local prior_matrices : e(matrices)
local prior_functions : e(functions)
foreach item of local prior_scalars {
    local prior_scalar_`item' : display %21x e(`item')
}
foreach item of local prior_macros {
    local prior_macro_`item' `"`e(`item')'"'
}
local prior_matrix_index 0
foreach item of local prior_matrices {
    local ++prior_matrix_index
    tempname prior_matrix_`prior_matrix_index'
    matrix `prior_matrix_`prior_matrix_index'' = e(`item')
    local prior_matrix_name_`item' `prior_matrix_`prior_matrix_index''
}

foreach stage in posthold postplugin poststripe postereturn postscalars postdiagnostics precommit {
    capture noisily _stfixest_estimate_ols_test y c.x1, ///
        absorb(worker firm) cluster(cluster_a cluster_b) ///
        residual(__stfx_failed_resid) inject(`stage')
    local observed_rc = _rc
    assert `observed_rc' == 3498
    confirm new variable __stfx_failed_resid

    local after_scalars : e(scalars)
    local after_macros : e(macros)
    local after_matrices : e(matrices)
    local after_functions : e(functions)
    local missing : list prior_scalars - after_scalars
    local extra : list after_scalars - prior_scalars
    assert `"`missing'`extra'"' == ""
    local missing : list prior_macros - after_macros
    local extra : list after_macros - prior_macros
    assert `"`missing'`extra'"' == ""
    local missing : list prior_matrices - after_matrices
    local extra : list after_matrices - prior_matrices
    assert `"`missing'`extra'"' == ""
    local missing : list prior_functions - after_functions
    local extra : list after_functions - prior_functions
    assert `"`missing'`extra'"' == ""

    foreach item of local prior_scalars {
        local after_scalar : display %21x e(`item')
        assert `"`after_scalar'"' == `"`prior_scalar_`item''"'
    }
    foreach item of local prior_macros {
        assert `"`e(`item')'"' == `"`prior_macro_`item''"'
    }
    foreach item of local prior_matrices {
        local before_matrix `"`prior_matrix_name_`item''"'
        assert mreldif(e(`item'), `before_matrix') == 0
    }
    assert e(sample) == __stfx_prior_sample
}

* A real scientific error after hold follows the same restoration path.
capture noisily _stfixest_estimate_ols_test y c.x1 if 0, ///
    absorb(worker firm) residual(__stfx_empty_resid)
assert _rc != 0
confirm new variable __stfx_empty_resid
assert `"`e(cmd)'"' == "regress"
assert e(sample) == __stfx_prior_sample

* Null e() is also preserved for every boundary.
ereturn clear
local null_scalars : e(scalars)
local null_macros : e(macros)
local null_matrices : e(matrices)
local null_functions : e(functions)
foreach stage in posthold postplugin poststripe postereturn postscalars postdiagnostics precommit {
    capture noisily _stfixest_estimate_ols_test y c.x1, ///
        absorb(worker firm) vce(robust) ///
        residual(__stfx_null_resid) inject(`stage')
    assert _rc == 3498
    confirm new variable __stfx_null_resid
    local after_scalars : e(scalars)
    local after_macros : e(macros)
    local after_matrices : e(matrices)
    local after_functions : e(functions)
    assert `"`after_scalars'"' == `"`null_scalars'"'
    assert `"`after_macros'"' == `"`null_macros'"'
    assert `"`after_matrices'"' == `"`null_matrices'"'
    assert `"`after_functions'"' == `"`null_functions'"'
}

* Successful commit renames the private residual only in the final nobreak
* block and leaves a complete hidden eclass result.
_stfixest_estimate_ols_test y c.x1, absorb(worker firm) ///
    cluster(cluster_a cluster_b) residual(__stfx_committed_resid)
assert `"`e(cmd)'"' == "_stfixest_estimate_ols_test"
assert e(stfixest_receipt)[1,16] == 1
assert e(sample) == 1
assert !missing(__stfx_committed_resid)
assert det(e(V)) < 0
assert e(stfixest_certificate)[31,1] == 1
assert e(stfixest_certificate)[31,2] == det(e(V))
assert e(stfixest_certificate)[32,1] == 1
assert e(stfixest_certificate)[32,2] == 1

* Intercept-only and K0 posting, plus the no-absorb virtual structural
* constant under noconstant, use distinct exact e() layouts.
_stfixest_estimate_ols_test y, absorb(worker firm)
assert colsof(e(b)) == 1 & colsof(e(V)) == 1
local intercept_name : colfullnames e(b)
assert `"`intercept_name'"' == "_cons"
assert e(df_m) == 0 & e(df_a) == 18 & e(df_r) == 6

_stfixest_estimate_ols_test y, absorb(worker firm) noconstant
capture confirm matrix e(b)
assert _rc == 111
capture confirm matrix e(V)
assert _rc == 111
assert e(df_m) == 0 & e(df_a) == 18 & e(df_r) == 6
assert e(sample) == 1
assert e(stfixest_receipt)[1,5] == 0
assert e(stfixest_receipt)[1,6] == 0
assert e(stfixest_certificate)[31,1] == 1
assert e(stfixest_certificate)[31,2] == 1
assert e(stfixest_certificate)[32,2] == 1

* The manifest adapter must not fail after the inner K0 transaction commits.
_stfixest_ols_hidden_numeric_v1 y, absorb(worker firm) noconstant
capture confirm matrix e(b)
assert _rc == 111
capture confirm matrix e(V)
assert _rc == 111
assert `"`e(cmd)'"' == "_stfixest_ols_hidden_numeric_v1"
assert e(stfixest_v_determinant) == 1
assert e(stfixest_receipt)[1,5] == 0

_stfixest_estimate_ols_test y c.x1, noconstant
assert colsof(e(b)) == 1 & colsof(e(V)) == 1
local noconstant_name : colfullnames e(b)
assert `"`noconstant_name'"' == "x1"
assert e(df_m) == 1 & e(df_a) == 1 & e(df_r) == 22
assert e(stfixest_fe)[1,1] == 2

di as result "END_FIXTURE|estimate_ols_eclass_transaction|rc=0"
