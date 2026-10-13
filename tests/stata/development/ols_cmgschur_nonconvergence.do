version 18.0
clear all
set more off

* A connected graph large enough to require multiple preconditioned iterations.
set obs 12000
generate long worker = ceil(_n/2)
bysort worker: generate byte period = _n
generate int firm = 1 + mod(floor((worker-1)/6) + ///
    (period-1)*(1 + 6*mod(worker,6)), 1000)
generate double x = sin(_n*.31) + cos(worker*.017)
generate double y = .7*x + sin(worker*.11) + cos(firm*.3) + ///
    (mod(_n*37, 101) - 50)*.001

* Establish that the same valid model succeeds and consumes more than one step.
quietly fereg y x, absorb(worker firm) solver(cmgschur) ///
    cmgdegree(5) tolerance(1e-10) iterate(1000) threads(2)
assert e(fereg_solver)[1,2] == 8
assert e(fereg_solver)[1,3] > 1

quietly regress y x if worker <= 500
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
}

foreach mode in fast low {
    capture noisily fereg y x, absorb(failed_worker=worker failed_firm=firm) ///
        solver(cmgschur) cmgdegree(5) tolerance(1e-10) iterate(1) ///
        memory(`mode') poolsize(1) threads(2) residuals(failed_residual)
    assert _rc == 430
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
    assert e(sample) == prior_sample
}

display as result "END_FIXTURE|ols_cmgschur_nonconvergence|rc=0"
