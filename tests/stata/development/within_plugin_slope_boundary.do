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

forvalues case = 1/3 {
    use `source', clear
    keep if case == `case'
    assert _N == 128 & row == _n
    local expected_names "q v 3b.g 11o.g 29o.g 97o.g _cons"
    if `case' == 3 local expected_names "q o.v 3b.g 11o.g 29o.g 97o.g _cons"
    foreach workers in 1 4 {
        forvalues repeat = 1/2 {
            quietly fereg y q v i.g [aw=w], absorb(g##c.(z1 z2)) ///
                solver(lsmr) preconditioner(off) memory(fast) ///
                tolerance(1e-10) iterate(100) threads(`workers') deterministic ///
                residuals(actual_r) xbd(actual_f)
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
            assert (ref_v_energy <= 1e-9) == (`case' == 3)

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
