version 18.0
clear all
set more off
set seed 271828
set obs 32772
generate long row = _n
generate byte g = mod(_n,11)
generate byte h = mod(floor((_n-1)/11),3)
generate byte cluster = mod(floor((_n-1)/33),17)
generate double w = 1 + mod(_n,7)
generate double z = rnormal()
generate double y = g/10 + h/20 + rnormal()
forvalues j=1/16 {
    generate double x`j' = rnormal()
    quietly replace y = y + x`j'/`j'
}
quietly replace x16 = .a if mod(_n,997)==0
quietly replace w = .z if mod(_n,991)==0
quietly replace cluster = .b if mod(_n,983)==0
generate str4 cluster_string = string(cluster)
quietly replace cluster_string = "" if missing(cluster)

// Stata's explicit dummy regression is independent of native preparation,
// absorption and covariance. Cluster groups cross every absorbed group.
foreach case in iid_aw robust_none robust_aw robust_fw robust_pw cluster_none cluster_fw cluster_pw interaction_aw slope_aw {
    local weight
    local vce robust
    local absorption g
    local dummies i.g
    local restriction if mod(row,5)!=0
    if strpos("`case'","_aw") local weight [aw=w]
    if strpos("`case'","_fw") local weight [fw=w]
    if strpos("`case'","_pw") local weight [pw=w]
    if "`case'"=="iid_aw" local vce conventional
    if strpos("`case'","cluster_")==1 {
        local vce cluster cluster
        local restriction if mod(row,5)!=0 & !missing(cluster)
    }
    if "`case'"=="interaction_aw" {
        local absorption g#h
        local dummies i.g#i.h
    }
    if "`case'"=="slope_aw" {
        local absorption g##c.z
        local dummies i.g##c.z
    }
    local oracle_vce
    if "`vce'"!="conventional" local oracle_vce vce(`vce')
    quietly regress y x1-x16 `dummies' `restriction' `weight', `oracle_vce'
    matrix oracle_b=e(b)
    matrix oracle_v=e(V)
    scalar oracle_N=e(N)
    scalar oracle_rss=e(rss)
    generate byte oracle_sample=e(sample)
    predict double oracle_residual if e(sample), residuals
    foreach t in 1 4 16 {
        local options `oracle_vce'
        if strpos("`case'","cluster_")==1 local options vce(cluster cluster_string)
        quietly fereg y x1-x16 `restriction' `weight', absorb(`absorption') `options' threads(`t') residuals(native_residual)
        assert row==_n
        assert e(sample)==oracle_sample
        assert e(N)==oracle_N
        assert abs(e(rss)-oracle_rss)<1e-8*oracle_rss
        assert abs(native_residual-oracle_residual)<1e-7 if oracle_sample
        assert missing(native_residual) if !oracle_sample
        forvalues j=1/16 {
            assert abs(e(b)[1,`j']-oracle_b[1,`j'])<1e-8
            forvalues k=1/16 {
                assert abs(e(V)[`j',`k']-oracle_v[`j',`k'])<1e-9
            }
        }
        drop native_residual
    }
    drop oracle_sample oracle_residual
}
display as result "END_FIXTURE|ols_owned_preparation|rc=0"
