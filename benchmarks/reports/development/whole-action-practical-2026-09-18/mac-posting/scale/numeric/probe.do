version 18.0
clear all
set more off
set processors 1
set segmentsize 1m
program define snap
    args phase
    quietly memory
    file open f using "probe.tsv", write append
    foreach s in data_data_a data_data_u data_strl_a data_strl_u data_etc_a data_etc_u data_oh_a data_oh_u stata_matrices_a stata_ado_a stata_sr_a mata_matrices_a mata_functions_a maxvar_a other_a {
        file write f "`phase'" _tab "`s'" _tab %24.0f (r(`s')) _n
    }
    file close f
end
program define publish, eclass
    tempvar sample residual
    generate byte `sample'=1
    snap sample
    generate double `residual'=.
    snap residual
    replace `residual'=1
    snap filled
    snap pre_fvexpand
    fvexpand x1 if `sample'
    snap post_fvexpand
    tempname b V held
    tempvar held_sample
    matrix `b'=J(1,2,1)
    matrix colnames `b'=x1 _cons
    matrix `V'=I(2)
    matrix colnames `V'=x1 _cons
    matrix rownames `V'=x1 _cons
    _estimates hold `held', restore nullok varname(`held_sample')
    snap held
    ereturn post `b' `V', obs(`=_N') esample(`sample') buildfvinfo
    snap posted
    rename `residual' saved_e
    _estimates unhold `held', not
    snap unheld
end
capture erase probe.tsv
set obs 1048576
forvalues j=1/1 {
    generate double x`j'=_n
}
generate int year=2001+mod(_n,1)
snap input
publish
snap returned
assert saved_e==1
assert e(sample)==1
file open f using "result.txt", write replace
file write f "STATA_MEMORY_PROBE_PASS" _n
file close f
exit, clear
