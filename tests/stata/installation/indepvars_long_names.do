version 15.0
clear all
set more off

* Exercise the option quoting boundary without any estimator or comparator.
* Syntax/base slots are removed; numerical omissions and _cons are retained.
program define check_names
    args n skipbase
    local names
    local expected
    tempname status
    matrix `status' = J(`n',4,0)
    forvalues k = 1/`=`n'-1' {
        local names `"`names' 1.half#`k'.categorical_control_longname"'
        if `skipbase' & mod(`k',2) == 1 matrix `status'[`k',2] = 2
        else local expected `"`expected' 1bn.half#`k'bn.categorical_control_longname"'
    }
    local names = strtrim(`"`names' _cons"')
    local expected = strtrim(`"`expected' _cons"')
    _fereg_indepvars, target(actual) count(`n') status(`status') names(`"`names'"')
    assert `"`actual'"' == `"`expected'"'
end
foreach n in 3 10 100 234 235 236 300 377 1000 {
    check_names `n' 0
    check_names `n' 1
}
matrix S = J(7,4,0)
matrix S[2,2] = 2
_fereg_indepvars, target(actual) count(7) status(S) ///
    names("x 0b.g 1.g o.z 2o.g#c.x L.x _cons")
assert `"`actual'"' == "x 1bn.g z 2bn.g#c.x L.x _cons"
_fereg_indepvars, target(actual) count(0) status(no_such_matrix)
assert `"`actual'"' == ""

* A real long factor axis used to fail with r(132) after fitting successfully.
* Dummy OLS supplies an independent coefficient witness on the same sample.
quietly set obs 4000
generate long categorical_control_longname = mod(_n,300)+1
generate int panel = mod(floor((_n-1)/300),7)
generate double x = sin(_n/7)
generate double y = 2*x + categorical_control_longname/50 + panel + sin(_n/3)
quietly regress y x i.categorical_control_longname i.panel, vce(robust)
scalar reference_x = _b[x]
scalar reference_se = _se[x]
quietly fereg y x i.categorical_control_longname, absorb(panel) vce(robust)
assert abs(_b[x]-reference_x) < 1e-10
assert abs(_se[x]-reference_se) < 1e-10
local expected x
forvalues level = 2/300 {
    local expected `"`expected' `level'bn.categorical_control_longname"'
}
local expected `"`expected' _cons"'
assert `"`e(indepvars)'"' == `"`expected'"'
local columns : colnames e(b)
assert strpos(`"`columns'"', "1b.categorical_control_longname") > 0
tempname posted_b
matrix `posted_b' = e(b)
assert colsof(`posted_b') == 302
predict double score, xb
quietly _predict double reference_score if e(sample), xb
assert reldif(score,reference_score) < 1e-12

* Small public OLS and PPML fits exercise the shared base-slot adaptation.
clear
sysuse auto, clear
quietly fereg price weight i.foreign
assert `"`e(indepvars)'"' == "weight 1bn.foreign _cons"
quietly feppml price weight i.foreign
assert `"`e(indepvars)'"' == "weight 1bn.foreign"

// The PPML status wire distinguishes explicit syntax omissions from bases.
quietly feppml price weight o.length 1o.foreign
assert `"`e(indepvars)'"' == "weight"
generate byte twin = foreign
quietly feppml price weight ibn.foreign#ibn.twin
assert !strpos(`"`e(indepvars)'"', "0bn.foreign#1bn.twin")
assert !strpos(`"`e(indepvars)'"', "1bn.foreign#0bn.twin")

di as result "END_FIXTURE|indepvars_long_names|rc=0"
