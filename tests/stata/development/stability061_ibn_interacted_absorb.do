version 18.0
clear all
set more off
set obs 1728
generate long obs_id = _n
generate byte half = mod(_n-1,2)
generate byte c01 = 1+mod(floor((_n-1)/2),3)
generate byte c02 = 1+mod(floor((_n-1)/6),2)
generate byte period = 1+mod(floor((_n-1)/12),3)
generate byte entry = 1+mod(floor((_n-1)/36),3)
generate byte family = 1+mod(floor((_n-1)/108),2)
generate byte replicate = floor((_n-1)/216)
generate double x = sin(_n*.61)+replicate/19
generate double pw = .7+mod(_n,7)/5
generate int cl1 = 1+mod(_n,19)
generate int cl2 = 1+mod(floor((_n-1)/5),23)
generate double y = exp(.2*x+.3*half*(c01==3)-.15*(1-half)*(c01==3) ///
    +.1*half*(c02==2)+.04*period-.03*entry+.02*family) ///
    *(.8+.1*mod(replicate,3)+.05*sin(_n/7))

* Independent design: use c01=2 and c02=1 within each half as reference.
* All absorbed dummy columns are explicit; no fereg factor parser is used.
forvalues h=0/1 {
    generate byte d13_`h' = half==`h' & c01==3
    generate byte d22_`h' = half==`h' & c02==2
    generate byte d11_`h' = half==`h' & c01==1
}
local design x d13_0 d13_1 d22_0 d22_1 d11_0 d11_1
foreach source in period entry family {
    quietly levelsof `source', local(levels)
    foreach level of local levels {
        forvalues h=0/1 {
            generate byte fe_`source'`level'_`h' = half==`h' & `source'==`level'
            local design `design' fe_`source'`level'_`h'
        }
    }
}
tempfile original
save `original'

mata:
real matrix ibn_cluster_meat(real matrix score, real colvector group)
{
    real colvector levels
    real matrix sums
    real scalar j
    levels=uniqrows(sort(group,1))
    sums=J(rows(levels),cols(score),0)
    for(j=1;j<=rows(levels);j++) sums[j,.]=colsum(select(score,group:==levels[j]))
    return(quadcross(sums,sums))
}
void ibn_sandwich(string scalar vars, real scalar clustered)
{
    real matrix X, Hinv, score, meat, V
    real colvector use, y, mu, w, a, b
    use=selectindex(st_data(.,"sample"))
    X=st_data(use,tokens(vars)); y=st_data(use,"y")
    mu=st_data(use,"reference_mu"); w=st_data(use,"pw")
    Hinv=invsym(quadcross(X,w:*mu,X))
    score=X:*(w:*(y-mu))
    meat=quadcross(score,score)
    if(clustered) {
        a=st_data(use,"cl1"); b=st_data(use,"cl2")
        meat=ibn_cluster_meat(score,a)+ibn_cluster_meat(score,b) ///
            -ibn_cluster_meat(score,1000:*a+b)
    }
    V=Hinv*meat*Hinv*st_numscalar("correction")
    st_matrix("reference_V",V[1..5,1..5])
}
end

foreach model in ols ppml {
foreach scenario in full missing_half separated_half separated_base {
    if "`model'"=="ols" & strpos("`scenario'","separated") continue
    use `original', clear
    generate byte wanted = 1
    if "`scenario'"=="missing_half" {
        replace y = . if half==0
        replace wanted = half==1
    }
    if "`scenario'"=="separated_half" {
        replace y = 0 if half==0
        replace wanted = half==1
    }
    if "`scenario'"=="separated_base" {
        replace y = 0 if c01==1
        replace wanted = c01!=1
    }
    fvexpand x ibn.half#i.(c01 c02) if !missing(y)
    local original_names `r(varlist)' _cons
    local slots : word count `original_names'
    foreach covariance in robust cluster {
        if "`model'"=="ols" & "`covariance'"=="cluster" continue
        local vce robust
        if "`covariance'"=="cluster" local vce cluster cl1 cl2
        if "`model'"=="ppml" {
            quietly feppml y x ibn.half#i.(c01 c02) [pw=pw], ///
                absorb(period#half entry#half family#half) vce(`vce') ///
                mu(actual) threads(1)
            matrix status = e(feppml_status)
            scalar correction = e(feppml_inference)[3,1]
        }
        else {
            quietly fereg y x ibn.half#i.(c01 c02) [pw=pw], ///
                absorb(period#half entry#half family#half) vce(robust) ///
                residuals(actual) threads(1)
            matrix status = e(fereg_status)
        }
        assert e(sample)==wanted
        generate byte sample = e(sample)
        assert missing(actual) if !sample
        matrix b = e(b)
        matrix V = e(V)
        assert colsof(b)==`slots'
        local posted : colnames b
        matrix L = J(5,`slots',0)
        matrix L[1,1] = 1
        * Check every stripe's original source/level/base identity, including
        * vanished levels. Numerical omission markers may be added, not bases.
        forvalues j=2/`=`slots'-1' {
            local original : word `j' of `original_names'
            quietly _ms_parse_parts `original'
            local h = r(level1)
            local level = r(level2)
            local source `r(name2)'
            local base1 = r(base1)
            local base2 = r(base2)
            local actual_name : word `j' of `posted'
            quietly _ms_parse_parts `actual_name'
            assert "`r(name1)'"=="half" & "`r(name2)'"=="`source'"
            assert r(level1)==`h' & r(level2)==`level'
            assert r(base1)==`base1' & r(base2)==`base2'
            if "`source'"=="c01" & `level'==3 matrix L[2+`h',`j']=1
            if "`source'"=="c01" & `level'==2 matrix L[2+`h',`j']=-1
            if "`source'"=="c02" & `level'==2 matrix L[4+`h',`j']=1
            if "`source'"=="c02" & `level'==1 matrix L[4+`h',`j']=-1
        }
        forvalues j=1/`slots' {
            if inlist(status[`j',2],1,5) continue
            assert b[1,`j']==0
            forvalues k=1/`slots' {
                assert V[`j',`k']==0 & V[`k',`j']==0
            }
        }
        matrix contrast_b=L*b'
        matrix contrast_V=L*V*L'
        if "`model'"=="ppml" {
            quietly poisson y `design' [pw=pw] if sample, noconstant nolog
            predict double reference_mu if sample, n
            assert abs(actual-reference_mu)<1e-7*(1+reference_mu) if sample
            mata: ibn_sandwich("`design'", "`covariance'"=="cluster")
        }
        else {
            quietly regress y `design' [pw=pw] if sample, noconstant vce(robust)
            predict double reference_mu if sample, residuals
            assert abs(actual-reference_mu)<1e-8 if sample
            matrix reference_V=e(V)[1..5,1..5]
        }
        matrix reference_b=e(b)[1,1..5]'
        forvalues j=1/5 {
            if inlist("`scenario'","missing_half","separated_half") & inlist(`j',2,4) continue
            assert abs(contrast_b[`j',1]-reference_b[`j',1])<1e-7
            forvalues k=1/5 {
                if inlist("`scenario'","missing_half","separated_half") & inlist(`k',2,4) continue
                assert abs(contrast_V[`j',`k']-reference_V[`j',`k'])<1e-7
            }
        }
        drop actual sample reference_mu
    }
}
}

* A source may simultaneously carry a default base and all-level recipes.
* Absorbed slopes use the same original-axis admission policy.
foreach absorption in "period#half entry#half family#half" ///
    "period#half entry#half family#half period#c.z" {
    foreach model in ols ppml {
        use `original', clear
        generate double z = cos(obs_id*.23)
        local reference_design `design'
        if strpos("`absorption'","c.z") {
            forvalues p=1/3 {
                generate double slope`p' = (period==`p')*z
                local reference_design `reference_design' slope`p'
            }
        }
        if "`model'"=="ppml" {
            quietly feppml y x i.half ibn.half#i.(c01 c02) [pw=pw], ///
                absorb(`absorption') vce(robust) mu(actual)
            scalar correction=e(feppml_inference)[3,1]
        }
        else {
            quietly fereg y x i.half ibn.half#i.(c01 c02) [pw=pw], ///
                absorb(`absorption') vce(robust) residuals(actual)
        }
        assert e(sample)==1
        matrix b=e(b)
        matrix V=e(V)
        generate byte sample=e(sample)
        if "`model'"=="ppml" {
            quietly poisson y `reference_design' [pw=pw], noconstant nolog
            predict double reference_mu, n
            mata: ibn_sandwich("`reference_design'",0)
        }
        else {
            quietly regress y `reference_design' [pw=pw], noconstant vce(robust)
            predict double reference_mu, residuals
            matrix reference_V=e(V)[1..5,1..5]
        }
        assert abs(actual-reference_mu)<1e-7*(1+abs(reference_mu))
        assert abs(b[1,1]-_b[x])<1e-7
        assert abs(V[1,1]-reference_V[1,1])<1e-7
    }
}

* Exact parenthesized range spelling, including duplicate finite columns.
use `original', clear
forvalues j=3/30 {
    local name = "c"+string(`j',"%02.0f")
    generate byte `name' = c02
}
order c01 c02 c03-c30
quietly feppml y ibn.half#i.(c01-c30) [pw=pw], ///
    absorb(period#half entry#half family#half) mu(range_mu)
assert e(sample)==1
assert colsof(e(b))==123
quietly poisson y d13_0 d13_1 d22_0 d22_1 d11_0 d11_1 fe_* [pw=pw], noconstant nolog
predict double reference_mu, n
assert abs(range_mu-reference_mu)<1e-7*(1+reference_mu)

* Other explicit base policies still fail atomically, even beside valid ibn.
quietly regress y x
matrix held_b=e(b)
matrix held_V=e(V)
generate byte held_sample=e(sample)
foreach command in fereg feppml {
    local output residuals(unpublished)
    if "`command'"=="feppml" local output mu(unpublished)
    foreach rhs in "ib0.half#i.c01" "ib1.half#i.c01" ///
        "ibn.half#ib1.c01" "ibn.half#ib2.c01" {
        capture noisily `command' y `rhs', absorb(period#half) `output'
        assert _rc==498
        assert "`e(cmd)'"=="regress"
        assert mreldif(e(b),held_b)==0 & mreldif(e(V),held_V)==0
        assert e(sample)==held_sample
        confirm new variable unpublished
    }
    foreach base in 1 2 {
        fvset base `base' c01
        capture noisily `command' y ibn.half#i.c01, absorb(period#half) `output'
        assert _rc==498
        assert "`e(cmd)'"=="regress"
        assert mreldif(e(b),held_b)==0 & mreldif(e(V),held_V)==0
        assert e(sample)==held_sample
        confirm new variable unpublished
        fvset clear c01
    }
}

display as result "END_FIXTURE|stability061_ibn_interacted_absorb|rc=0"
