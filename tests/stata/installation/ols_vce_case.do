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

* Balanced 3-by-3 grid. The interaction slope scores in y_cross sum to zero
* within either dimension, making their two-way inclusion-exclusion variance
* negative. The additive slope scores in y_add have positive two-way variance.
clear
set obs 18
generate byte a = floor((_n-1)/6)-1
generate byte b = mod(floor((_n-1)/2),3)-1
generate byte x = 2*mod(_n,2)-1
generate double y_cross = (1+a*b)*x + a+b
generate double y_add = (1+a+b)*x + a-b
foreach case in clipped positive oneway robust {
    local y = cond("`case'" == "positive", "y_add", "y_cross")
    local vce "cluster a b"
    if "`case'" == "oneway" local vce "cluster a"
    if "`case'" == "robust" local vce "robust"
    tempfile transcript
    log using "`transcript'", text replace name(psd)
    noisily fereg `y' x, vce(`vce')
    noisily fereg
    log close psd
    local note "Cameron-Gelbach-Miller adjustment"
    local count = (strlen(fileread("`transcript'")) - ///
        strlen(subinstr(fileread("`transcript'"), "`note'", "", .))) / strlen("`note'")
    assert `count' == 2*("`case'" == "clipped")
    if "`case'" == "robust" assert missing(e(fereg_psd_adjusted))
    else assert e(fereg_psd_adjusted) == ("`case'" == "clipped")
}

* Estimates from before the diagnostic existed can still be displayed.
quietly regress y_cross x, vce(cluster a)
noisily _fereg_display
assert "`e(cmd)'" == "regress"

* The earlier certificate is sufficient when the new scalar is absent.
program define fit_with_legacy_psd_metadata, eclass
    quietly fereg y_cross x, vce(cluster a b)
    ereturn scalar fereg_psd_adjusted = .
end
fit_with_legacy_psd_metadata
tempfile legacy_transcript
log using "`legacy_transcript'", text replace name(psd)
noisily fereg
log close psd
assert strpos(fileread("`legacy_transcript'"), "Cameron-Gelbach-Miller adjustment") > 0

display as result "END_FIXTURE|ols_vce_case|rc=0"
