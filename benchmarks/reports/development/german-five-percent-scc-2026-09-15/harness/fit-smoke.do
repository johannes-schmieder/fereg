version 18.0
clear all
set more off
set varabbrev off
set linesize 240
args runroot input out family arm threads
set processors `threads'
sysdir set PLUS "`out'/empty-plus"
sysdir set PERSONAL "`out'/empty-personal"
foreach package in require ftools reghdfe ppmlhdfe {
    adopath ++ "`runroot'/references/`package'/src"
}
adopath ++ "`runroot'/source/stata"
adopath ++ "`runroot'/source/dist"
use "`input'", clear
assert obs_id==_n
assert inrange(year,2001,2009)
assert !missing(persid,betnr,logrwage2,age)
assert inlist(edyrs,10,13,16,18,.)
generate double wage=exp(logrwage2)
assert wage>0 & wage<.
generate byte edu_group=cond(missing(edyrs),99,edyrs)
local controls
foreach e in 10 13 16 18 99 {
    generate double age2_ed`e'=((age-40)^2/100)*(edu_group==`e')
    generate double age3_ed`e'=((age-40)^3/1000)*(edu_group==`e')
    local controls `controls' age2_ed`e' age3_ed`e'
}
local original_n=_N
which fereg
which feppml
which ppmlhdfe
which reghdfe
fereg, version
if "`family'"=="ppml" {
    if "`arm'"=="native" local command feppml wage `controls' i.year, absorb(persid betnr) vce(robust) d(absorbed) solver(auto) threads(`threads') memory(fast) poolsize(4) timeit
    else local command ppmlhdfe wage `controls' i.year, absorb(persid betnr) vce(robust) d(absorbed) tolerance(1e-8)
}
else if "`family'"=="ols" {
    if "`arm'"=="native" local command fereg logrwage2 `controls' i.year, absorb(persid betnr) residuals(residual) solver(auto) threads(`threads') memory(fast) timeit
    else local command reghdfe logrwage2 `controls' i.year, absorb(persid betnr) residuals(residual) tolerance(1e-8)
}
else exit 198
timer clear
timer on 1
capture noisily `command'
local fitrc=_rc
timer off 1
timer list 1
local seconds=r(t1)
file open result using "`out'/fit-result.txt", write replace
file write result "FIT_RC=`fitrc'" _n "COMMAND_SECONDS=`seconds'" _n
file close result
if `fitrc'!=0 exit `fitrc'
assert _N==`original_n'
assert obs_id==_n
generate byte sample=e(sample)
count if sample
assert r(N)==e(N)
if "`family'"=="ppml" {
    assert e(converged)==1
    predict double fitted_mu if sample, mu
    assert fitted_mu>0 & fitted_mu<. if sample
    assert missing(fitted_mu) if !sample
}
else {
    generate double fitted_outcome=logrwage2-residual if sample
    assert !missing(residual,fitted_outcome) if sample
}
ereturn list
file open scalars using "`out'/scalars.tsv", write replace
foreach key in `: e(scalars)' {
    file write scalars "`key'" _tab %24.17e (e(`key')) _n
}
file close scalars
local available_matrices: e(matrices)
foreach key in b V fereg_status fereg_fit fereg_dof fereg_certificate feppml_status feppml_fit feppml_dof feppml_drop_counts feppml_solver feppml_certificate feppml_runtime {
    if strpos(" `available_matrices' ", " `key' ") {
        matrix vals=e(`key')
        local names: colfullnames vals
        local rows: rowfullnames vals
        file open mat using "`out'/`key'.tsv", write replace
        file write mat "row"
        foreach name of local names {
            file write mat _tab "`name'"
        }
        file write mat _n
        forvalues i=1/`=rowsof(vals)' {
            local name: word `i' of `rows'
            file write mat "`name'"
            forvalues j=1/`=colsof(vals)' {
                file write mat _tab %24.17e (vals[`i',`j'])
            }
            file write mat _n
        }
        file close mat
    }
}
estimates save "`out'/estimates.ster", replace
if "`family'"=="ppml" keep obs_id persid betnr year logrwage2 age edyrs sample absorbed fitted_mu
else keep obs_id persid betnr year logrwage2 age edyrs sample residual fitted_outcome
save "`out'/outputs.dta", replace
display as result "GERMAN_FIVE_PERCENT_FIT_COMPLETE|family=`family'|arm=`arm'"
