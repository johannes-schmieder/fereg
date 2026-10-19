version 18.0
clear all
set more off
args workers
if `"`workers'"' == "" local workers 4
assert inlist(real(`"`workers'"'), 2, 4)

* Two disconnected weak rings with one degree-eight worker in each component.
* Each cell has 32 opposite pairs, so within-x and the noise are orthogonal.
set obs 140
generate long cell = _n - 1
generate byte component = floor(cell/70)
generate byte local_cell = mod(cell,70)
generate byte local_worker = cond(local_cell < 8, 0, 1 + floor((local_cell-8)/2))
generate byte neighbor = cond(local_cell < 8, local_cell, mod(local_cell-8,2))
generate long worker = 32*component + local_worker
generate long firm = 32*component + mod(local_worker+neighbor,32)
expand 64
bysort cell: generate byte pair = floor((_n-1)/2)
by cell: generate byte sign = 2*mod(_n-1,2)-1
generate double weight = (1+mod(3*worker+firm,7))/8
generate double aw = (mod(17*worker+3,37)-18)/8
generate double af = (mod(13*firm+5,43)-21)/16
generate double x = sign*(1-2*mod(pair,2))*(1+mod(worker+firm,3))/8 + 2*aw - 3*af
generate double truth = sign*(1-2*mod(floor(pair/2),2))*(1+mod(2*worker+firm,5))/16
generate double y = .75*x + aw + af + truth

quietly _fereg_estimate_ols_test y x, absorb(worker firm) aweight(weight) ///
    vce(robust) solver(auto) memory(fast) tolerance(1e-12) threads(`workers') residual(reference)
assert e(fereg_solver)[1,3] > 4
assert abs(_b[x]-.75) < 1e-9
assert abs(reference-truth) < 1e-8
generate byte prior_sample = e(sample)
local prior_scalars : e(scalars)
local prior_macros : e(macros)
local prior_matrices : e(matrices)
local prior_functions : e(functions)
* Numeric indices keep every snapshot local within Stata's name-length limit.
local scalar_index 0
foreach item of local prior_scalars {
    local ++scalar_index
    local prior_scalar_`scalar_index' : display %21x e(`item')
}
local macro_index 0
foreach item of local prior_macros {
    local ++macro_index
    local prior_macro_`macro_index' `"`e(`item')'"'
}
local matrix_index 0
foreach item of local prior_matrices {
    local ++matrix_index
    tempname prior_matrix_`matrix_index'
    matrix `prior_matrix_`matrix_index'' = e(`item')
}
local b_index : list posof "b" in prior_matrices
local v_index : list posof "V" in prior_matrices
assert `b_index' > 0 & `v_index' > 0

* The default route needs more than four iterations, so the hidden synchronous
* h4/k1 builder starts. Each marker follows its first cancellation poll after
* executed construction bookkeeping; it does not establish execution of a
* parallel numerical kernel. A second interrupted action proves that the
* developer Break claim and the cancellation flag were reset.
forvalues attempt=1/2 {
    capture noisily _fereg_estimate_ols_test y x, ///
        absorb(failed_worker=worker failed_firm=firm) aweight(weight) ///
        vce(robust) solver(auto) memory(fast) tolerance(1e-12) threads(`workers') ///
        residual(failed_residual) inject(routing_builder_break)
    assert _rc == 1
    foreach output in failed_worker failed_firm failed_residual {
        confirm new variable `output'
    }
    local after_scalars : e(scalars)
    local after_macros : e(macros)
    local after_matrices : e(matrices)
    local after_functions : e(functions)
    foreach kind in scalars macros matrices functions {
        local missing : list prior_`kind' - after_`kind'
        local extra : list after_`kind' - prior_`kind'
        assert `"`missing'`extra'"' == ""
    }
    local scalar_index 0
    foreach item of local prior_scalars {
        local ++scalar_index
        local value : display %21x e(`item')
        assert `"`value'"' == `"`prior_scalar_`scalar_index''"'
    }
    local macro_index 0
    foreach item of local prior_macros {
        local ++macro_index
        assert `"`e(`item')'"' == `"`prior_macro_`macro_index''"'
    }
    local matrix_index 0
    foreach item of local prior_matrices {
        local ++matrix_index
        assert mreldif(e(`item'), `prior_matrix_`matrix_index'') == 0
    }
    assert e(sample) == prior_sample
}

* An ordinary action must recover exact incumbent bits after cancellation.
quietly _fereg_estimate_ols_test y x, absorb(worker firm) aweight(weight) ///
    vce(robust) solver(auto) memory(fast) tolerance(1e-12) threads(`workers') residual(reset)
assert reset == reference
assert mreldif(e(b), `prior_matrix_`b_index'') == 0
assert mreldif(e(V), `prior_matrix_`v_index'') == 0

* The same process must also be able to build and install a hierarchy again.
quietly _fereg_estimate_ols_test y x, absorb(reset_worker=worker reset_firm=firm) ///
    aweight(weight) vce(robust) solver(auto) memory(fast) tolerance(1e-12) ///
    threads(`workers') residual(followup) inject(routing_h8_k1)
assert abs(_b[x]-.75) < 1e-9
assert abs(followup-truth) < 1e-8
assert e(sample) == prior_sample
di as result "FEREG_BUILDER_" "BREAK_RESET_VERIFIED"
di as result "END_" "FIXTURE|estimate_ols_builder_break|rc=0"
