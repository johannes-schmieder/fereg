version 18.0
clear all
set more off
set seed 938271
set obs 2048
generate long row_id=_n
generate byte g1=mod(_n,8)
generate byte g2=mod(floor((_n-1)/8),7)
generate byte g3=mod(floor((_n-1)/56),5)
generate byte c1=mod(_n,17)
generate byte f=mod(floor((_n-1)/3),3)
generate double w=1+mod(_n,5)
generate double z=rnormal()
forvalues j=1/3 {
    generate double x`j'=rnormal()+g1/10
}
generate double x4=2*x1
generate double y=0.7*x1-0.2*x2+0.1*x3+g1/10+g2/8+g3/9+0.1*z*g1+0.3*rnormal()
replace x3=.z in 19
foreach case in two three slope generated {
    local rhs x1-x4
    local absorb g1 g2
    local dummies i.g1 i.g2
    local weight [aw=w]
    local vce robust
    if "`case'"=="three" {
        local absorb g1 g2 g3
        local dummies i.g1 i.g2 i.g3
        local weight [pw=w]
        local vce cluster c1
    }
    if "`case'"=="slope" {
        local absorb g1##c.z
        local dummies i.g1##c.z
    }
    if "`case'"=="generated" {
        local rhs i.f##c.x1 x2 x3
        local weight
    }
    quietly regress y `rhs' `dummies' `weight', vce(`vce')
    matrix oracle_b=e(b)
    matrix oracle_V=e(V)
    scalar oracle_df=e(df_r)
    predict double oracle_e, residuals
    generate byte expected=e(sample)
    quietly fereg y `rhs' `weight', absorb(`absorb') vce(`vce') memory(low) poolsize(2) threads(4) residuals(native_e)
    assert e(sample)==expected
    assert row_id==_n
    assert e(df_r)==oracle_df
    assert abs(native_e-oracle_e)<1e-8 if expected
    matrix actual_b=e(b)
    matrix actual_V=e(V)
    local names : colnames actual_b
    local j=0
    foreach name of local names {
        local ++j
        if "`name'"!="_cons" {
            local oj=colnumb(oracle_b,"`name'")
            assert `oj'<.
            assert abs(actual_b[1,`j']-oracle_b[1,`oj'])<1e-8
            local k=0
            foreach other of local names {
                local ++k
                if "`other'"!="_cons" {
                    local ok=colnumb(oracle_b,"`other'")
                    assert `ok'<.
                    assert abs(actual_V[`j',`k']-oracle_V[`oj',`ok'])<1e-9
                }
            }
        }
    }
    display "LOW_SCORE_LAYOUT|case=`case'|rc=0"
    drop expected native_e oracle_e
}
display "END_FIXTURE|low_score_layout|fits=4|rc=0"
