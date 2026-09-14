version 18.0
clear all
set more off
set seed 938271
set obs 8193
generate long obs_id=_n
generate byte g1=mod(_n,8)
generate byte g2=mod(floor((_n-1)/8),7)
generate byte g3=mod(floor((_n-1)/56),5)
generate byte c1=mod(_n,23)
generate byte f=floor(4*runiform())
generate byte h=floor(8*runiform())
generate double w=1+mod(_n,5)
generate double z=rnormal()
generate double off=0.1*sin(_n/21)
generate double exposure=exp(off)
forvalues j=1/8 {
    generate double x`j'=rnormal()+g1/10
}
generate double y=0.7*x1-0.2*x2+0.1*x3+f/10+h/8+g1/10+g2/8+g3/9+0.1*z*g1+0.3*rnormal()
replace x8=.z in 19
replace h=. in 71
replace w=. in 32
replace c1=. in 98
foreach case in aw_fast pw_low fw_three unit_slope {
    local rhs x1-x8 i.f##i.h
    local absorb g1
    local dummies i.g1
    local weight [aw=w]
    local vce robust
    local options
    if "`case'"=="pw_low" {
        local weight [pw=w]
        local vce cluster c1
        local options memory(low) poolsize(2)
    }
    if "`case'"=="fw_three" {
        local absorb g1 g2 g3
        local dummies i.g1 i.g2 i.g3
        local weight [fw=w]
        local vce conventional
    }
    if "`case'"=="unit_slope" {
        local weight
        local absorb g1##c.z
        local dummies i.g1##c.z
    }
    local oracle_vce vce(`vce')
    if "`vce'"=="conventional" local oracle_vce
    quietly regress y `rhs' `dummies' `weight', `oracle_vce'
    matrix oracle_b=e(b)
    matrix oracle_V=e(V)
    scalar oracle_df=e(df_r)
    predict double oracle_e, residuals
    generate byte expected=e(sample)
    local native_vce vce(`vce')
    if "`vce'"=="conventional" local native_vce
    quietly fereg y `rhs' `weight', absorb(`absorb') `native_vce' `options' threads(4) residuals(native_e)
    assert e(sample)==expected
    assert obs_id==_n
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
    display "GENERATED_ORACLE|case=`case'|rc=0"
    drop expected native_e oracle_e
}
replace y=1+mod(_n,17)
quietly poisson y x1-x8 i.f##i.h i.g1 [fw=w], exposure(exposure) vce(robust) nolog
matrix oracle_b=e(b)
matrix oracle_V=e(V)
predict double oracle_mu, n
generate byte expected=e(sample)
local oracle_names x1 x2 x3 x4 x5 x6 x7 x8
forvalues j=1/3 {
    local oracle_names `oracle_names' `j'.f
}
forvalues j=1/7 {
    local oracle_names `oracle_names' `j'.h
}
forvalues j=1/3 {
    forvalues k=1/7 {
        local oracle_names `oracle_names' `j'.f#`k'.h
    }
}
forvalues j=0/7 {
    local oracle_names `oracle_names' fe`j'
}
mata:
f=st_data(.,"f"); h=st_data(.,"h"); g=st_data(.,"g1")
X=st_data(.,tokens("x1 x2 x3 x4 x5 x6 x7 x8"))
for (j=1;j<=3;j++) X=(X,f:==j)
for (j=1;j<=7;j++) X=(X,h:==j)
for (j=1;j<=3;j++) for (k=1;k<=7;k++) X=(X,(f:==j):*(h:==k))
for (j=0;j<=7;j++) X=(X,g:==j)
keep=st_data(.,"expected"):==1
X=select(X,keep); y=select(st_data(.,"y"),keep); mu=select(st_data(.,"oracle_mu"),keep); w=select(st_data(.,"w"),keep)
Hinv=invsym(quadcross(X,w:*mu,X))
V=Hinv*quadcross(X,w:*(y-mu):^2,X)*Hinv*(sum(w)/(sum(w)-1))
st_matrix("independent_V",V)
stripe=J(cols(X),2,""); stripe[.,2]=tokens(st_local("oracle_names"))'
st_matrixcolstripe("independent_V",stripe)
st_matrixrowstripe("independent_V",stripe)
st_numscalar("independent_correction",sum(w)/(sum(w)-1))
end
quietly feppml y x1-x8 i.f##i.h [fw=w], absorb(g1) exposure(exposure) vce(robust) separation(none) keepsingletons threads(4) mu(native_mu)
assert e(sample)==expected
assert obs_id==_n
assert e(converged)==1
assert e(feppml_certificate)[16,1]==1
assert abs(e(feppml_inference)[3,1]-independent_correction)<1e-14
assert abs(native_mu-oracle_mu)<=1e-7*(1+abs(oracle_mu)) if expected
matrix actual_b=e(b)
matrix actual_V=e(V)
local names : colnames actual_b
local j=0
foreach name of local names {
    local ++j
    if "`name'"!="_cons" {
        local oj=colnumb(oracle_b,"`name'")
        local ov=colnumb(independent_V,"`name'")
        if `ov'>=. {
            assert actual_b[1,`j']==0
            forvalues k=1/`=colsof(actual_b)' {
                assert actual_V[`j',`k']==0
            }
            continue
        }
        assert `oj'<.
        assert abs(actual_b[1,`j']-oracle_b[1,`oj'])<1e-7
        local k=0
        foreach other of local names {
            local ++k
            if "`other'"!="_cons" {
                local ok=colnumb(independent_V,"`other'")
                if `ok'>=. {
                    assert actual_V[`j',`k']==0
                }
                else {
                    assert abs(actual_V[`j',`k']-independent_V[`ov',`ok'])<1e-7
                }
            }
        }
    }
}
display "GENERATED_ORACLE|case=ppml_fw|rc=0"
display "END_FIXTURE|generated_rhs|fits=5|rc=0"
