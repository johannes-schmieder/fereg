version 18
args expect_schur
clear all
set more off
set obs 18432
generate long row=_n
generate byte g1=1+mod(_n,8)
generate byte g2=1+mod(floor((_n-1)/8),5)
generate byte cl=1+mod(floor((_n-1)/40),7)
generate double w=1+mod(_n,7)
generate double x1=sin(_n*.371)
generate double x2=cos(_n*.619)
generate double y=10+.4*g1-.1*g2+1.2*x1-.7*x2+.05*sin(_n*1.337)
forvalues j=3/16 {
    generate double x`j'=sin(_n*(.131+.017*`j'))
    quietly replace y=y+x`j'/`j'
}
order row g1 g2 cl w y x1 x2 x3-x16
replace x2=.a if mod(row,233)==0
replace w=.b if mod(row,229)==0
replace cl=.c if mod(row,227)==0
local fits=0
foreach dimension in 1 2 {
    local dummies i.g1
    local absorption native_g1=g1
    if `dimension'==2 {
        local dummies i.g1 i.g2
        local absorption native_g1=g1 native_g2=g2
    }
    foreach kind in unit aw fw pw {
        local weight
        local mean_weight
        local internal_weight
        local vce robust
        if "`kind'"!="unit" {
            local weight [`kind'=w]
            local mean_weight [aw=w]
            if "`kind'"=="aw" local internal_weight aweight(w)
            if "`kind'"=="fw" local internal_weight fweight(w)
            if "`kind'"=="pw" local internal_weight pweight(w)
        }
        if "`kind'"=="pw" local vce cluster cl
        quietly regress y x1-x16 `dummies' if mod(row,11)!=0 `weight', vce(`vce')
        matrix oracle_b=e(b)
        matrix oracle_v=e(V)
        generate byte oracle_sample=e(sample)
        predict double oracle_residual if oracle_sample, residuals
        foreach mode in fast low {
            quietly fereg y x1-x16 if mod(row,11)!=0 `weight', absorb(`absorption') vce(`vce') memory(`mode') threads(4) residuals(native_e)
            if `expect_schur' & `dimension'==2 & "`mode'"=="fast" {
                assert e(fereg_solver)[1,15]==8
                assert e(fereg_solver)[1,16]<=.01
            }
            assert row==_n
            assert e(sample)==oracle_sample
            assert abs(_b[x1]-oracle_b[1,1])<1e-9
            assert abs(_b[x2]-oracle_b[1,2])<1e-9
            forvalues j=1/16 {
                forvalues k=1/16 {
                    assert abs(e(V)[`j',`k']-oracle_v[`j',`k'])<1e-9
                }
            }
            assert abs(native_e-oracle_residual)<1e-8 if oracle_sample
            assert missing(native_e) & missing(native_g1) if !oracle_sample
            generate double oracle_d=y-oracle_residual-_b[x1]*x1-_b[x2]*x2-_b[_cons] if oracle_sample
            forvalues j=3/16 {
                assert abs(_b[x`j']-oracle_b[1,`j'])<1e-9
                quietly replace oracle_d=oracle_d-_b[x`j']*x`j' if oracle_sample
            }
            generate double native_d=native_g1
            if `dimension'==2 quietly replace native_d=native_d+native_g2
            assert abs(native_d-oracle_d)<1e-8 if oracle_sample
            forvalues j=1/`dimension' {
                quietly summarize native_g`j' if oracle_sample `mean_weight', meanonly
                assert abs(r(mean))<1e-9
                quietly levelsof g`j', local(levels)
                foreach level of local levels {
                    quietly summarize native_g`j' if oracle_sample & g`j'==`level', meanonly
                    assert r(max)==r(min)
                }
            }
            matrix full_b=e(b)
            matrix full_v=e(V)
            quietly fereg y x1-x16 if mod(row,11)!=0 `weight', absorb(partial_g1=g1 `=cond(`dimension'==2,"g2", "")') vce(`vce') memory(`mode') threads(4)
            assert mreldif(e(b),full_b)==0
            assert mreldif(e(V),full_v)==0
            assert partial_g1==native_g1
            local internal_vce vce(robust)
            if "`kind'"=="pw" local internal_vce cluster(cl)
            quietly _fereg_estimate_ols y x1-x16 if mod(row,11)!=0, absorb(combined_g1=g1 `=cond(`dimension'==2,"combined_g2=g2", "")') `internal_weight' `internal_vce' memory(`mode') threads(4) d(combined_d) xb(combined_xb) xbd(combined_xbd) residual(combined_e)
            assert abs(combined_g1-native_g1)<1e-10 if oracle_sample
            assert abs(combined_d-native_d)<1e-10 if oracle_sample
            assert abs(combined_e-native_e)<1e-10 if oracle_sample
            assert abs(combined_xbd-combined_xb-combined_d)<1e-10 if oracle_sample
            assert abs(y-combined_xbd-combined_e)<1e-8 if oracle_sample
            assert missing(combined_d) if !oracle_sample
            drop native_g* native_e native_d oracle_d partial_g1 combined_*
            local fits=`fits'+3
        }
        drop oracle_sample oracle_residual
    }
}
matrix previous_b=e(b)
capture noisily _fereg_estimate_ols y x1-x16, absorb(failed_saved=g1) inject(precommit)
local failure_rc=_rc
assert `failure_rc'==3498
assert mreldif(e(b),previous_b)==0
capture confirm variable failed_saved
assert _rc==111
assert row==_n
display "SAVED_SCHUR_ORACLE|fits=" `fits' "|rollback=1|rc=0"
display "END_FIXTURE|ols_saved_schur|rc=0"
exit, STATA clear
