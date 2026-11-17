version 15.0
clear all
set more off

quietly set obs 120
generate long id = _n
generate byte g = mod(_n,6)
generate double x = sin(_n/7)
generate double y = 2*x + g + sin(_n/3)
generate double count = 2 + mod(_n,5)

* This runs after native completion and after ereturn post. Test both an
* ordinary helper error and the return code used by Stata's user Break.
program define _fereg_indepvars, eclass
    ereturn local partial_public_metadata "must disappear on failure"
    exit $metadata_failure_rc
end

foreach prior in regress null {
    foreach failure in 3498 1 {
        global metadata_failure_rc `failure'
        if "`prior'" == "regress" quietly regress y x if id <= 90
        else ereturn clear
        tempvar prior_sample
        if "`prior'" == "regress" generate byte `prior_sample' = e(sample)
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
        foreach item of local prior_matrices {
            tempname prior_matrix_`item'
            matrix `prior_matrix_`item'' = e(`item')
        }
        unab prior_variables : _all
        foreach command in fereg feppml {
            local lhs = cond("`command'" == "fereg", "y", "count")
            capture noisily `command' `lhs' x, ///
                absorb(failed_fe=g) residuals(failed_residual)
            assert _rc == `failure'
            confirm new variable failed_fe failed_residual
            unab after_variables : _all
            assert `"`after_variables'"' == `"`prior_variables'"'
            assert id == _n
            foreach kind in scalars macros matrices functions {
                local after : e(`kind')
                local missing : list prior_`kind' - after
                local extra : list after - prior_`kind'
                assert `"`missing'`extra'"' == ""
            }
            foreach item of local prior_scalars {
                local after_scalar : display %21x e(`item')
                assert `"`after_scalar'"' == `"`prior_scalar_`item''"'
            }
            foreach item of local prior_macros {
                assert `"`e(`item')'"' == `"`prior_macro_`item''"'
            }
            foreach item of local prior_matrices {
                tempname after_matrix
                matrix `after_matrix' = e(`item')
                assert mreldif(`after_matrix', `prior_matrix_`item'') == 0
            }
            if "`prior'" == "regress" assert e(sample) == `prior_sample'
        }
        capture drop `prior_sample'
    }
}
macro drop metadata_failure_rc
program drop _fereg_indepvars

* Failure after all public fields have been assembled still restores the
* previous result and leaves every requested output name uncreated.
quietly regress y x if id <= 90
tempname prior_b prior_v after_b after_v
matrix `prior_b' = e(b)
matrix `prior_v' = e(V)
tempvar prior_sample
generate byte `prior_sample' = e(sample)
capture noisily _fereg_estimate_ols y x, absorb(failed_fe=g) ///
    residual(failed_residual) publicmode publiccmdline("fereg y x, absorb(g)") ///
    publicsolver(auto) inject(precommit)
assert _rc == 3498
assert `"`e(cmd)'"' == "regress"
matrix `after_b' = e(b)
matrix `after_v' = e(V)
assert mreldif(`after_b',`prior_b') == 0 & mreldif(`after_v',`prior_v') == 0
assert e(sample) == `prior_sample'
confirm new variable failed_fe failed_residual
capture confirm matrix e(fereg_receipt)
assert _rc == 111
drop `prior_sample'

* The public contract recovers after a failed publication, including saved
* output names, a quoted weight expression and all-zero finite width.
quietly fereg y x [aw=cond(g==1,2,1)], absorb(saved_fe=g) residuals(saved_residual) ///
    solver(lsmr, precondition(jacobi)) memory(low) threads(1) ///
    poolsize(3) tolerance(1e-9) iterate(1000) deterministic timeit verbose(2)
assert `"`e(cmd)'"' == "fereg"
assert `"`e(indepvars)'"' == "x _cons"
assert `"`e(fe_requested)'"' == "lsmr, precondition(jacobi)"
assert `"`e(fe_requested_preconditioner)'"' == "jacobi"
assert `"`e(fereg_memory_requested)'"' == "low"
assert e(fereg_poolsize) == 3 & e(fereg_requested_tolerance) == 1e-9
assert e(fereg_iterate) == 1000 & e(fereg_verbose) == 2
assert e(fereg_timeit) == 1 & e(fereg_deterministic) == 1
assert strpos(`"`e(cmdline)'"', "[aw=cond(g==1,2,1)]") > 0
confirm variable saved_fe saved_residual
predict double fitted, xbd
assert abs(y-fitted-saved_residual) < 1e-12
quietly fereg y, absorb(g) noconstant
assert `"`e(cmd)'"' == "fereg"
assert `"`e(indepvars)'"' == ""
capture confirm matrix e(b)
assert _rc == 111

* Preserve a long original command line containing inner string quotes across
* the private publication option; quoted expression parsing must not truncate.
local long_if `"if "quoted words" == "quoted words""'
forvalues repetition = 1/150 {
    local long_if `"`long_if' & "quoted words" == "quoted words""'
}
local long_command `"fereg y x `long_if', absorb(g) threads(1)"'
quietly `long_command'
assert `"`e(cmdline)'"' == `"`long_command'"'
quietly fereg y, absorb(g) noconstant

* Private publication context cannot be partially supplied; validation must
* happen without replacing these successful public estimates.
foreach context in "publicmode" "publiccmdline(fereg y)" "publicsolver(auto)" "publictimeit" "publicverbose(2)" {
    capture noisily _fereg_estimate_ols y x, `context' residual(failed_residual)
    assert _rc == 198
    confirm new variable failed_residual
    assert `"`e(cmd)'"' == "fereg"
    assert `"`e(indepvars)'"' == ""
}

di as result "END_FIXTURE|public_metadata_transaction|rc=0"
