version 18.0
clear all
set more off
set obs 300
generate long id = _n
generate int g = ceil(_n/20)
generate byte f = mod(_n,3)+1
generate byte h = mod(floor(_n/3),2)+1
generate double x = sin(_n*.31)+cos(_n*.17)
generate double y = .8*x+.3*(f==2)-.2*(f==3)+.1*(h==2)+ ///
    .2*(f==3)*(h==2)+.03*g+sin(_n*.73)
generate double w = 1+mod(_n,5)
generate int cl = ceil(_n/10)
generate byte one = 1
tempfile source
save `source'

foreach scenario in complete restricted missing_y missing_x missing_level missing_fw missing_cluster singleton aw pw {
    use `source', clear
    local restriction
    local weight
    local inference vce(robust)
    local absorption g
    if "`scenario'" == "restricted" local restriction if id<=240
    if "`scenario'" == "missing_y" replace y = . in 1
    if "`scenario'" == "missing_x" replace x = . in 1
    if "`scenario'" == "missing_level" {
        replace y = . if f==1
        // Stata excludes unavailable rows before parsing; its original base
        // is already level2 here and must remain unchanged.
        local absorption g#one
    }
    if "`scenario'" == "missing_fw" {
        replace w = . in 1
        local weight [fw=w]
    }
    if "`scenario'" == "missing_cluster" {
        replace cl = . in 1
        local inference vce(cluster cl)
    }
    if "`scenario'" == "singleton" replace g = 99 in 1
    if "`scenario'" == "aw" local weight [aw=w]
    if "`scenario'" == "pw" local weight [pw=w]

    quietly fereg y x i.f##i.h `restriction' `weight', absorb(`absorption') `inference' threads(4)
    local unchanged = inlist("`scenario'", "complete", "restricted", "aw", "pw")
    assert (e(fereg_receipt)[1,8] == e(fereg_receipt)[1,9]) == `unchanged'
    assert e(fereg_receipt)[1,9] == e(fereg_dof)[4,2]
    if "`scenario'" == "missing_level" {
        local names : colfullnames e(b)
        assert strpos("`names'", "2b.f") > 0
        assert strpos("`names'", "1b.f") == 0
    }
    matrix b = e(b)
    matrix V = e(V)
    matrix status = e(fereg_status)
    generate byte sample = e(sample)
    local names : colnames b
    // Dummy OLS supplies independent finite coefficients and the sandwich.
    // Cluster FE are not nested here, so the full rank correction agrees.
    quietly regress y x i.f##i.h i.g if sample `weight', `inference'
    forvalues j=1/`=colsof(b)' {
        if status[`j',2] != 1 continue
        local name : word `j' of `names'
        local ref = colnumb(e(b), "`name'")
        assert `ref' < .
        assert abs(b[1,`j']-e(b)[1,`ref']) < 1e-8
        forvalues k=1/`=colsof(b)' {
            if status[`k',2] != 1 continue
            local other : word `k' of `names'
            local refk = colnumb(e(b), "`other'")
            assert abs(V[`j',`k']-e(V)[`ref',`refk']) < 1e-8
        }
    }
}
display "END_FIXTURE|ols_factor_name_reuse|rc=0"
