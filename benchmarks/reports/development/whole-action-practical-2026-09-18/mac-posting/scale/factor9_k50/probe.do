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
    fvexpand x1 x2 x3 x4 x5 x6 x7 x8 x9 x10 x11 x12 x13 x14 x15 x16 x17 x18 x19 x20 x21 x22 x23 x24 x25 x26 x27 x28 x29 x30 x31 x32 x33 x34 x35 x36 x37 x38 x39 x40 x41 x42 x43 x44 x45 x46 x47 x48 x49 x50 i.year if `sample'
    snap post_fvexpand
    tempname b V held
    tempvar held_sample
    matrix `b'=J(1,60,1)
    matrix colnames `b'=x1 x2 x3 x4 x5 x6 x7 x8 x9 x10 x11 x12 x13 x14 x15 x16 x17 x18 x19 x20 x21 x22 x23 x24 x25 x26 x27 x28 x29 x30 x31 x32 x33 x34 x35 x36 x37 x38 x39 x40 x41 x42 x43 x44 x45 x46 x47 x48 x49 x50 2001b.year 2002.year 2003.year 2004.year 2005.year 2006.year 2007.year 2008.year 2009.year _cons
    matrix `V'=I(60)
    matrix colnames `V'=x1 x2 x3 x4 x5 x6 x7 x8 x9 x10 x11 x12 x13 x14 x15 x16 x17 x18 x19 x20 x21 x22 x23 x24 x25 x26 x27 x28 x29 x30 x31 x32 x33 x34 x35 x36 x37 x38 x39 x40 x41 x42 x43 x44 x45 x46 x47 x48 x49 x50 2001b.year 2002.year 2003.year 2004.year 2005.year 2006.year 2007.year 2008.year 2009.year _cons
    matrix rownames `V'=x1 x2 x3 x4 x5 x6 x7 x8 x9 x10 x11 x12 x13 x14 x15 x16 x17 x18 x19 x20 x21 x22 x23 x24 x25 x26 x27 x28 x29 x30 x31 x32 x33 x34 x35 x36 x37 x38 x39 x40 x41 x42 x43 x44 x45 x46 x47 x48 x49 x50 2001b.year 2002.year 2003.year 2004.year 2005.year 2006.year 2007.year 2008.year 2009.year _cons
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
forvalues j=1/50 {
    generate double x`j'=_n
}
generate int year=2001+mod(_n,9)
snap input
publish
snap returned
assert saved_e==1
assert e(sample)==1
file open f using "result.txt", write replace
file write f "STATA_MEMORY_PROBE_PASS" _n
file close f
exit, clear
