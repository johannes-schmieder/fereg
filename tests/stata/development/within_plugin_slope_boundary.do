version 18.0
clear all
set more off

* Prospective exact-input oracle and acceptance bounds:
* bench/results/within-plugin-slope-boundary-declaration.json.
* Explicit LSMR is required: one-FE Auto would exercise MAP instead.
quietly findfile fereg.ado
local ado_path = subinstr(`"`r(fn)'"', "\", "/", .)
local suffix "/stata/fereg.ado"
assert substr(`"`ado_path'"', -strlen(`"`suffix'"'), .) == `"`suffix'"'
local root = substr(`"`ado_path'"', 1, strlen(`"`ado_path'"') - strlen(`"`suffix'"'))
import delimited using `"`root'/tests/fixtures/ols/within_plugin_slope_boundary.csv"', varnames(1) asdouble stringcols(25/27) clear
foreach variable in z1 z2 y {
    assert strtrim(string(`variable',"%21x")) == hex_`variable'
}
assert inlist(g,3,11,29,97) & inlist(w,2,3,5,7)
assert inlist(q,-1,1) & inlist(v,-1,1)
tempfile source
save `source'

forvalues case = 1/4 {
    use `source', clear
    keep if case == `case'
    assert _N == 128 & row == _n
    if inlist(`case',2,3) {
        * Preserve the approved existing rejection: both beta.10 and the
        * candidate failed their unchanged independent FE check in these cases.
        * The exact retained-span oracle remains in the CSV; it is not a
        * published estimate when fereg's independent certificate rejects.
        quietly regress y q v i.g [aw=w]
        generate byte prior_sample = e(sample)
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
            local prior_rows_`item' : rowfullnames e(`item')
            local prior_cols_`item' : colfullnames e(`item')
        }
        quietly datasignature set, reset
        foreach attempt in w1a w1b w4a w4b base {
            local workers = cond(substr("`attempt'",2,1) == "4",4,1)
            local rhs i.g
            local expected_rc 459
            if "`attempt'" == "base" {
                local rhs ib3.g
                local expected_rc 498
            }
            capture noisily fereg y q v `rhs' [aw=w], absorb(g##c.(z1 z2)) ///
                solver(lsmr) preconditioner(off) memory(fast) ///
                tolerance(1e-10) iterate(100) threads(`workers') deterministic ///
                residuals(actual_r)
            local observed_rc = _rc
            assert `observed_rc' == `expected_rc'
            confirm new variable actual_r
            confirm new variable actual_f
            quietly datasignature confirm
            foreach kind in scalars macros matrices functions {
                local after_`kind' : e(`kind')
                local missing : list prior_`kind' - after_`kind'
                local extra : list after_`kind' - prior_`kind'
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
                local before_matrix `"`prior_matrix_name_`item''"'
                assert mreldif(e(`item'),`before_matrix') == 0
                local after_rows : rowfullnames e(`item')
                local after_cols : colfullnames e(`item')
                assert `"`after_rows'"' == `"`prior_rows_`item''"'
                assert `"`after_cols'"' == `"`prior_cols_`item''"'
            }
            assert e(sample) == prior_sample
            display "BOUNDARY_REJECTION|case=`case'|attempt=`attempt'|workers=`workers'|rc=`observed_rc'|rollback=1"
        }
        continue
    }
    local expected_names "q v 3b.g 11o.g 29o.g 97o.g _cons"
    if `case' == 4 local expected_names "q o.v 3b.g 11o.g 29o.g 97o.g _cons"
    foreach workers in 1 4 {
        forvalues repeat = 1/2 {
            quietly fereg y q v i.g [aw=w], absorb(g##c.(z1 z2)) ///
                solver(lsmr) preconditioner(off) memory(fast) ///
                tolerance(1e-10) iterate(100) threads(`workers') deterministic ///
                residuals(actual_r)
            quietly predict double actual_f, xbd
            assert e(N) == 128 & e(N_full) == 128 & e(num_singletons) == 0
            assert e(sample) == 1 & e(rank) == ref_rank & e(converged) == 1
            assert "`e(fereg_solver_used)'" == "lsmr"
            assert e(fereg_solver)[1,1] == 3 & e(fereg_solver)[1,2] == 3
            assert e(fereg_solver)[1,3] > 0 & e(fereg_solver)[1,11] == 0
            assert e(fereg_certificate)[32,2] == 1
            local names : colfullnames e(b)
            assert `"`names'"' == `"`expected_names'"'
            assert colsof(e(b)) == 7 & rowsof(e(fereg_status)) == 7
            assert e(fereg_status)[1,2] == 1 & e(fereg_status)[2,2] == ref_v_status
            assert e(fereg_status)[3,2] == 2 & e(fereg_status)[7,2] == 5
            forvalues j = 1/7 {
                assert e(fereg_status)[`j',1] == `j'
                if inrange(`j',4,6) assert e(fereg_status)[`j',2] == 3
                if inrange(e(fereg_status)[`j',2],2,4) {
                    assert e(b)[1,`j'] == 0
                    forvalues k = 1/7 {
                        assert e(V)[`j',`k'] == 0 & e(V)[`k',`j'] == 0
                    }
                }
            }
            assert abs(e(b)[1,1]-ref_bq) <= 1e-8
            assert abs(e(b)[1,2]-ref_bv) <= 1e-8
            assert abs(e(b)[1,7]-ref_cons) <= 1e-8
            assert abs(e(rss)-ref_rss) <= 1e-8*ref_rss
            assert abs(actual_r-ref_resid) <= 1e-8
            assert abs(actual_f-ref_fitted) <= 1e-8
            assert (ref_v_energy <= 1e-9) == (`case' == 4)

            * Independent input-space moments use exact-oracle basis columns,
            * not the plugin's reported certificate or returned FE coefficients.
            foreach level in 3 11 29 97 {
                foreach column in fe0 fe1 fe2 {
                    generate double moment = w*actual_r*`column'*(g == `level')
                    quietly summarize moment, meanonly
                    assert abs(r(sum))/ref_ynorm <= 1e-9
                    drop moment
                }
            }
            foreach column in finite_q finite_v {
                generate double moment = w*actual_r*`column'
                quietly summarize moment, meanonly
                assert abs(r(sum))/ref_ynorm <= 1e-9
                drop moment
            }
            matrix signature = e(fereg_solver)
            forvalues j = 1/2 {
                matrix signature[`j',9] = 0
                matrix signature[`j',10] = 0
            }
            if `repeat' == 1 {
                matrix worker_b = e(b)
                matrix worker_v = e(V)
                matrix worker_status = e(fereg_status)
                matrix worker_signature = signature
                generate double worker_r = actual_r
                generate double worker_f = actual_f
            }
            else {
                assert mreldif(e(b),worker_b) == 0 & mreldif(e(V),worker_v) == 0
                assert mreldif(e(fereg_status),worker_status) == 0
                assert mreldif(signature,worker_signature) == 0
                assert actual_r == worker_r & actual_f == worker_f
                drop worker_r worker_f
            }
            if `workers' == 1 & `repeat' == 1 {
                matrix cross_b = e(b)
                matrix cross_v = e(V)
                matrix cross_status = e(fereg_status)
                matrix cross_signature = signature
                generate double cross_r = actual_r
                generate double cross_f = actual_f
            }
            if `workers' == 4 {
                assert mreldif(e(b),cross_b) <= 1e-8 & mreldif(e(V),cross_v) <= 1e-8
                assert mreldif(e(fereg_status),cross_status) == 0
                assert abs(actual_r-cross_r) <= 1e-8 & abs(actual_f-cross_f) <= 1e-8
                * Both absorption and final-residual rows: requested/used
                * solver, stop, fallback, warning count and preconditioners.
                * Iterations, timing and certificate magnitudes may differ.
                forvalues j = 1/2 {
                    foreach k in 1 2 4 11 12 13 14 {
                        assert signature[`j',`k'] == cross_signature[`j',`k']
                    }
                }
            }
            display "BOUNDARY_CASE|case=`case'|workers=`workers'|repeat=`repeat'|rank=" e(rank) "|method=`e(fereg_solver_used)'"
            drop actual_r actual_f
        }
    }
    * Explicit base overrides remain unsupported with sloped absorption.
    * Failure must preserve the accepted result and publish no new output.
    matrix accepted_b = e(b)
    matrix accepted_v = e(V)
    matrix accepted_status = e(fereg_status)
    capture noisily fereg y q v ib3.g [aw=w], absorb(g##c.(z1 z2)) ///
        solver(lsmr) preconditioner(off) residuals(should_not_exist)
    assert _rc == 498
    assert "`e(cmd)'" == "fereg"
    assert mreldif(e(b),accepted_b) == 0 & mreldif(e(V),accepted_v) == 0
    assert mreldif(e(fereg_status),accepted_status) == 0
    assert e(sample) == 1 & e(rank) == ref_rank
    confirm new variable should_not_exist
}
display as result "END_FIXTURE|within_plugin_slope_boundary|rc=0"
