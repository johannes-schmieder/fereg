version 18.0
clear all
set more off
set varabbrev off
set seed 290102
set obs 240

generate long row = _n
generate int Firm = ceil(_n / 8)
generate int firm = mod(_n, 12) + 1
generate byte Sector = mod(ceil(_n / 4), 5) + 1
generate double x = rnormal()
generate double y = 1 + 2*x + sin(Firm) + rnormal()
replace Firm = . in 1/3

* An independent one-way covariance and sample distinguish Firm from firm.
quietly regress y x, vce(cluster Firm)
matrix oracle_b = e(b)
matrix oracle_V = e(V)
scalar oracle_N = e(N)
scalar oracle_groups = e(N_clust)
generate byte oracle_sample = e(sample)
quietly regress y x, vce(cluster firm)
matrix lowercase_V = e(V)
assert mreldif(oracle_V, lowercase_V) > 1e-6
assert e(N) != oracle_N

foreach mode in fast low {
    foreach keyword in cluster CLuStEr {
        quietly fereg y x, vce(`keyword' Firm) memory(`mode') threads(1)
        assert `"`e(clustvar)'"' == "Firm"
        assert `"`e(clustvar1)'"' == "Firm"
        assert e(N) == oracle_N
        assert e(N_clust) == oracle_groups
        assert e(sample) == oracle_sample
        assert mreldif(e(b), oracle_b) < 1e-10
        assert mreldif(e(V), oracle_V) < 1e-10
        assert row == _n
    }
}
quietly fereg y x, vce(cluster firm) threads(1)
assert `"`e(clustvar)'"' == "firm"
assert mreldif(e(V), lowercase_V) < 1e-10

* Multiway and interacted identifiers reach the native adapter unchanged.
foreach clusters in "Firm Sector" "Firm#Sector" {
    quietly _fereg_estimate_ols y x, cluster(`clusters') threads(1)
    matrix adapter_b = e(b)
    matrix adapter_V = e(V)
    local labels `"`e(clustvar)'"'
    local dimensions = e(N_clustervars)
    quietly fereg y x, vce(CLUSTER `clusters') threads(1)
    assert `"`e(clustvar)'"' == `"`labels'"'
    assert e(N_clustervars) == `dimensions'
    assert e(sample) == oracle_sample
    assert mreldif(e(b), adapter_b) == 0
    assert mreldif(e(V), adapter_V) == 0
}

* Keyword normalization continues to accept all existing noncluster aliases.
foreach keyword in robust unadjusted ols iid {
    local reference = cond("`keyword'" == "robust", "robust", "ols")
    quietly regress y x, vce(`reference')
    matrix reference_b = e(b)
    matrix reference_V = e(V)
    local mixed = upper("`keyword'")
    quietly fereg y x, vce(`mixed') threads(1)
    assert mreldif(e(b), reference_b) < 1e-10
    assert mreldif(e(V), reference_V) < 1e-10
    matrix upper_b = e(b)
    matrix upper_V = e(V)
    quietly fereg y x, vce(`keyword') threads(1)
    assert mreldif(e(b), upper_b) == 0
    assert mreldif(e(V), upper_V) == 0
}

* Firm works when no lowercase counterpart exists.
rename firm lowercase_cluster
quietly fereg y x, vce(cluster Firm) threads(1)
assert `"`e(clustvar)'"' == "Firm"
assert mreldif(e(V), oracle_V) < 1e-10
rename Firm uppercase_cluster
rename lowercase_cluster firm

* A missing Firm must fail even when firm exists, preserving prior results/data.
quietly regress y x if row > 10
matrix prior_b = e(b)
matrix prior_V = e(V)
local prior_N = e(N)
local prior_df = e(df_r)
generate byte prior_sample = e(sample)
local prior_k = c(k)
foreach request in "cluster Firm" "cluster" "robust firm" "bogus" "cluster firm##Sector" {
    capture noisily fereg y x, vce(`request') residuals(forbidden_output)
    local rc = _rc
    assert `rc' != 0
    if "`request'" == "cluster Firm" assert `rc' == 111
    if inlist("`request'", "cluster", "robust firm", "bogus") assert `rc' == 198
    assert `"`e(cmd)'"' == "regress"
    assert e(N) == `prior_N'
    assert e(df_r) == `prior_df'
    assert mreldif(e(b), prior_b) == 0
    assert mreldif(e(V), prior_V) == 0
    assert e(sample) == prior_sample
    assert c(k) == `prior_k'
    assert row == _n
    confirm new variable forbidden_output
}

display as result "END_FIXTURE|ols_vce_case|rc=0"
