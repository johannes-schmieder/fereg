version 19.0
clear all
set more off
set varabbrev off
args refs output
set processors 4
sysdir set PLUS `"`refs'"'
sysdir set PERSONAL `"`output'"'
capture adopath - SITE
capture adopath - "."
adopath ++ `"`refs'"'
global GTOOLS_BETA "I KNOW WHAT I AM DOING"
set obs 600
generate long obs_id = _n
generate long g1 = ceil(_n/20)
generate long g2 = mod(_n-1, 10)+1
generate long cluster_id = mod(g1-1, 7)+1
generate double x1 = sin(_n*.31) + cos(_n*.17)
generate double x2 = cos(_n*.13) + sin(_n*.23)
generate double y = 3 + .3*x1 - .2*x2 + .1*sin(g1) + .1*cos(g2) + sin(_n*.73)
tempfile clean
save `clean'
tempname receipt
file open `receipt' using `"`output'/checks.tsv"', write text replace
file write `receipt' "case" _tab "N" _tab "sample_exact" _tab "b_V_unchanged" _n
forvalues case = 1/4 {
    use `clean', clear
    local effects g1
    local arm greg
    local options
    local selection
    generate byte expected = 1
    if `case' > 1 {
        local effects g1 g2
        local selection if obs_id <= 590 in 3/600
        replace y = . in 11
        replace x1 = . in 23
        replace g1 = . in 31
        replace expected = inrange(obs_id, 3, 590) & !missing(y, x1, x2, g1, g2)
    }
    if `case' == 3 {
        local options cluster(cluster_id)
        replace cluster_id = . in 41
        replace expected = expected & !missing(cluster_id)
    }
    if `case' == 4 {
        local arm gglm
        local options family(poisson) robust
        replace y = ceil(y) if !missing(y)
    }
    ereturn clear
    capture mata: mata drop BenchG
    global GTOOLS_GREGTABLE 0
    quietly `arm' y x1 x2 `selection', absorb(`effects') mata(BenchG) `options'
    local old_n = r(N)
    mata: st_matrix("B_old", BenchG.b)
    mata: st_matrix("V_old", BenchG.Vcov)
    ereturn clear
    capture mata: mata drop BenchG
    global GTOOLS_GREGTABLE 1
    quietly `arm' y x1 x2 `selection', absorb(`effects') mata(BenchG) `options'
    local new_n = r(N)
    mata: st_matrix("B_new", BenchG.b)
    mata: st_matrix("V_new", BenchG.Vcov)
    assert `old_n' == `new_n'
    assert e(N) == `new_n'
    assert e(sample) == expected
    count if e(sample)
    assert r(N) == `new_n'
    mata: assert(st_matrix("B_old") == st_matrix("B_new"))
    mata: assert(st_matrix("V_old") == st_matrix("V_new"))
    mata: assert(st_matrix("e(b)") == st_matrix("B_new"))
    mata: assert(st_matrix("e(V)") == st_matrix("V_new"))
    preserve
    keep if e(sample)
    keep obs_id
    export delimited using `"`output'/case-`case'-sample.csv"', replace
    restore
    file write `receipt' "`case'" _tab "`new_n'" _tab "1" _tab "1" _n
}
* A fresh failed invocation must not be mistaken for the preceding sample.
ereturn clear
capture noisily greg y x1 x2 if obs_id < 0, absorb(g1) mata(BenchG)
local failed_rc = _rc
capture confirm matrix e(b)
assert _rc != 0
file write `receipt' "empty_sample" _tab "`failed_rc'" _tab "no_e_b" _tab "not_ranked" _n
file close `receipt'
display as result "GTOOLS_SAMPLE_FIXTURE_COMPLETE"
exit 0
