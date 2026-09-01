version 18.0
local feppml_test_command `"$FEREG_PPML_TEST_COMMAND"'
local feppml_hidden_wrapper = strtrim(`"$FEREG_PPML_TEST_COMMAND"') != ""
if strtrim(`"`feppml_test_command'"') == "" local feppml_test_command "feppml"
clear all
set more off

* Frequency weights are literal observation replication for both the point
* estimate and the robust sandwich.
input long obs_id double(y x fw) byte literal_cluster
1 1 0 1 1
2 4 1 3 1
3 2 2 2 2
4 8 3 4 3
5 7 4 2 3
6 14 5 3 4
end

quietly summarize fw, meanonly
tempname fw_total
scalar `fw_total' = r(sum)
`feppml_test_command' y c.x [fw=fw], separation(none) vce(robust)
tempname fw_b fw_V
matrix `fw_b' = e(b)
matrix `fw_V' = e(V)
scalar __fw_effective_n = e(N_effective)
scalar __fw_sumweights = e(sumweights)
assert e(N_physical) == 6
assert e(N_effective) == scalar(`fw_total')
assert e(sumweights) == scalar(`fw_total')
assert `"`e(wtype)'"' == "fweight"

`feppml_test_command' y c.x [fw=fw], separation(none) vce(cluster literal_cluster)
tempname fw_cluster_b fw_cluster_V
matrix `fw_cluster_b' = e(b)
matrix `fw_cluster_V' = e(V)

expand fw
`feppml_test_command' y c.x, separation(none) vce(robust)
assert _N == scalar(__fw_effective_n)
assert e(N_physical) == scalar(__fw_effective_n)
assert e(N_effective) == scalar(__fw_effective_n)
assert e(sumweights) == scalar(__fw_sumweights)
assert mreldif(e(b), `fw_b') <= 1e-9
assert mreldif(e(V), `fw_V') <= 1e-8
`feppml_test_command' y c.x, separation(none) vce(cluster literal_cluster)
assert mreldif(e(b), `fw_cluster_b') <= 1e-9
assert mreldif(e(V), `fw_cluster_V') <= 1e-8

* Probability weights are normalized to physical N.  A common rescaling may
* change the raw total and multiplier, but not b, V, or effective N.
clear
set obs 40
generate long obs_id = _n
generate double x = mod(7 * _n, 17) / 4 - 2
generate double y = 0.25 + mod(5 * _n, 13)
generate double pw = 0.5 + mod(3 * _n, 11) / 3
generate double pw_scaled = 17 * pw
generate double pw_huge = 3e306 * pw
generate double pw_tiny = 1e-309 * pw
generate long pweight_cluster = 1 + mod(_n - 1, 8)

`feppml_test_command' y c.x [pw=pw], separation(none) vce(robust)
tempname pw_b pw_V
matrix `pw_b' = e(b)
matrix `pw_V' = e(V)
scalar __pw_raw = e(raw_sumweights)
scalar __pw_sum = e(sumweights)
scalar __pw_multiplier = e(weight_multiplier)
scalar __pw_log_raw = e(log_raw_sumweights)
scalar __pw_log_multiplier = e(log_weight_multiplier)
assert `"`e(wtype)'"' == "pweight"
assert e(N_physical) == _N
assert e(N_effective) == _N
assert reldif(e(sumweights), _N) <= 1e-12
assert e(feppml_objective_scale_code) == 1

`feppml_test_command' y c.x [pw=pw_scaled], separation(none) vce(robust)
assert mreldif(e(b), `pw_b') <= 1e-9
assert mreldif(e(V), `pw_V') <= 1e-8
assert e(N_physical) == _N
assert e(N_effective) == _N
assert reldif(e(sumweights), scalar(__pw_sum)) <= 1e-12
assert reldif(e(raw_sumweights), 17 * scalar(__pw_raw)) <= 1e-12
assert reldif(e(weight_multiplier), scalar(__pw_multiplier) / 17) <= 1e-12
assert reldif(e(log_raw_sumweights), scalar(__pw_log_raw) + ln(17)) <= 1e-12
assert reldif(e(log_weight_multiplier), scalar(__pw_log_multiplier) - ln(17)) <= 1e-12

* Every individual pweight is finite and positive in both extreme-scale
* cases. The computational experiment remains valid even when the raw sum or
* scalar normalization multiplier cannot be represented.
`feppml_test_command' y c.x [pw=pw_huge], separation(none) vce(robust)
assert mreldif(e(b), `pw_b') <= 1e-8
assert mreldif(e(V), `pw_V') <= 1e-7
assert reldif(e(sumweights), _N) <= 1e-12
capture confirm scalar e(raw_sumweights)
assert _rc == 111
assert e(log_raw_sumweights) < .
assert reldif(e(log_raw_sumweights), scalar(__pw_log_raw) + ln(3e306)) <= 1e-12
assert reldif(e(log_weight_multiplier), scalar(__pw_log_multiplier) - ln(3e306)) <= 1e-12
assert e(feppml_objective_scale_code) == 1

`feppml_test_command' y c.x [pw=pw_tiny], separation(none) vce(robust)
assert mreldif(e(b), `pw_b') <= 1e-8
assert mreldif(e(V), `pw_V') <= 1e-7
assert reldif(e(sumweights), _N) <= 1e-12
assert e(raw_sumweights) > 0 & e(raw_sumweights) < .
capture confirm scalar e(weight_multiplier)
assert _rc == 111
assert e(log_weight_multiplier) < .
assert reldif(e(log_raw_sumweights), scalar(__pw_log_raw) + ln(1e-309)) <= 1e-10
assert reldif(e(log_weight_multiplier), scalar(__pw_log_multiplier) - ln(1e-309)) <= 1e-10
assert e(feppml_objective_scale_code) == 1

`feppml_test_command' y c.x [pw=pw], separation(none) vce(cluster pweight_cluster)
tempname pw_cluster_b pw_cluster_V
matrix `pw_cluster_b' = e(b)
matrix `pw_cluster_V' = e(V)
`feppml_test_command' y c.x [pw=pw_scaled], separation(none) vce(cluster pweight_cluster)
assert mreldif(e(b), `pw_cluster_b') <= 1e-9
assert mreldif(e(V), `pw_cluster_V') <= 1e-8
assert e(N_effective) == _N

* The public adapter materializes legal weight expressions privately but keeps
* the parser-owned expression in e(wexp).  Bad and missing source values that
* are outside if/in are neither part of the sample nor grounds for rejection.
generate double fw_scope_source = 1 + mod(_n, 4)
replace fw_scope_source = -7 if obs_id > 32
replace fw_scope_source = . in 40
generate double fw_scope_materialized = 2 * fw_scope_source if obs_id <= 32

`feppml_test_command' y c.x [fw=2*fw_scope_source] if obs_id <= 32, ///
    separation(none) vce(robust)
tempname fw_expression_b fw_expression_V
matrix `fw_expression_b' = e(b)
matrix `fw_expression_V' = e(V)
assert e(N_physical) == 32
assert `"`e(wtype)'"' == "fweight"
assert `"`e(wexp)'"' == "= 2*fw_scope_source"
assert e(sample) == 0 if obs_id > 32

`feppml_test_command' y c.x [fw=fw_scope_materialized] if obs_id <= 32, ///
    separation(none) vce(robust)
assert mreldif(e(b), `fw_expression_b') <= 1e-9
assert mreldif(e(V), `fw_expression_V') <= 1e-8
assert e(sample) == 0 if obs_id > 32

generate double pw_scope_source = pw
replace pw_scope_source = -5 in 1/4
replace pw_scope_source = . in 37/40
generate double pw_scope_materialized = pw_scope_source / 7 in 5/36

`feppml_test_command' y c.x [pw=pw_scope_source/7] in 5/36, ///
    separation(none) vce(robust)
tempname pw_expression_b pw_expression_V
matrix `pw_expression_b' = e(b)
matrix `pw_expression_V' = e(V)
assert e(N_physical) == 32
assert `"`e(wtype)'"' == "pweight"
assert `"`e(wexp)'"' == "= pw_scope_source/7"
assert e(sample) == 0 if obs_id < 5 | obs_id > 36

`feppml_test_command' y c.x [pw=pw_scope_materialized] in 5/36, ///
    separation(none) vce(robust)
assert mreldif(e(b), `pw_expression_b') <= 1e-9
assert mreldif(e(V), `pw_expression_V') <= 1e-8
assert e(sample) == 0 if obs_id < 5 | obs_id > 36

* Candidate rows are not prefiltered by Stata's standard weight parser.  The
* native PPML domain owns exact error classification: invalid pweights are
* data-domain r(459), while invalid frequency weights are r(401).
tempname valid_weight_b valid_weight_V
matrix `valid_weight_b' = e(b)
matrix `valid_weight_V' = e(V)
generate byte __valid_weight_sample = e(sample)
generate double invalid_pw = pw
foreach bad_pw in 0 -1 {
    replace invalid_pw = `bad_pw' in 10
    capture noisily `feppml_test_command' y c.x [pw=invalid_pw], ///
        separation(none) residuals(__invalid_pw_resid)
    assert _rc == 459
    confirm new variable __invalid_pw_resid
    assert e(sample) == __valid_weight_sample
    assert mreldif(e(b), `valid_weight_b') == 0
    assert mreldif(e(V), `valid_weight_V') == 0
    replace invalid_pw = pw in 10
}
generate double invalid_fw = 1
foreach bad_fw in 0 -1 1.5 {
    replace invalid_fw = `bad_fw' in 10
    capture noisily `feppml_test_command' y c.x [fw=invalid_fw], ///
        separation(none) residuals(__invalid_fw_resid)
    assert _rc == 401
    confirm new variable __invalid_fw_resid
    assert e(sample) == __valid_weight_sample
    assert mreldif(e(b), `valid_weight_b') == 0
    assert mreldif(e(V), `valid_weight_V') == 0
    replace invalid_fw = 1 in 10
}

* Robust inference is asymptotic z/chi2, not OLS t/F.
`feppml_test_command' y c.x, separation(none) vce(robust)
tempname robust_b robust_V
matrix `robust_b' = e(b)
matrix `robust_V' = e(V)
assert e(V)[1,1] > 0 & e(V)[1,1] < .
assert e(V)[2,2] > 0 & e(V)[2,2] < .
assert e(df_resid) == _N - e(rank)
assert e(chi2) >= 0 & e(chi2) < .
test x
assert r(chi2) >= 0 & r(chi2) < .
assert r(df) == 1
assert r(p) >= 0 & r(p) <= 1

* vce(unadjusted) is the sole accepted public alias for robust PPML
* inference; it must not select an OLS/IID covariance route.
`feppml_test_command' y c.x, separation(none) vce(unadjusted)
assert `"`e(vce)'"' == "robust"
assert `"`e(vcetype)'"' == "Robust"
assert e(feppml_vce_requested) == 1
assert e(feppml_vce_effective) == 2
assert mreldif(e(b), `robust_b') <= 1e-9
assert mreldif(e(V), `robust_V') <= 1e-8

* Exercise every supported multiway clustered covariance path.  The receipt
* and subset table must expose all nonempty inclusion-exclusion subsets.
clear
set obs 360
generate long obs_id = _n
generate double x = mod(17 * _n, 37) / 9 - 2
generate double y = 0.5 + mod(11 * _n, 19)
generate long c1 = 1 + mod(_n - 1, 18)
generate long c2 = 1 + mod(floor((_n - 1) / 2), 15)
generate long c3 = 1 + mod(floor((_n - 1) / 3), 12)
generate long c4 = 1 + mod(floor((_n - 1) / 5), 9)
generate long c5 = 1 + mod(floor((_n - 1) / 7), 7)

local cluster_list
local minimum_groups "18 15 12 9 7"
forvalues dimensions = 1/5 {
    local cluster_list `cluster_list' c`dimensions'
    local expected_groups : word `dimensions' of `minimum_groups'
    `feppml_test_command' y c.x, separation(none) vce(cluster `cluster_list')
    assert `"`e(cmd)'"' == "feppml"
    assert `"`e(vce)'"' == "cluster"
    assert e(converged) == 1
    assert e(N_physical) == _N
    assert e(rank) == 2
    assert e(df_m) == 1
    assert e(df_structural) == _N - e(rank)
    assert e(df_resid) == `expected_groups' - 1
    assert rowsof(e(feppml_cluster)) == `dimensions'
    assert colsof(e(feppml_cluster)) == 4
    assert rowsof(e(feppml_subset)) == 2^`dimensions' - 1
    assert colsof(e(feppml_subset)) == 4
    assert e(feppml_receipt)[1,10] == `dimensions'
    assert e(feppml_receipt)[1,12] == 2^`dimensions' - 1
    assert e(feppml_receipt)[1,20] == 16
    assert e(feppml_receipt)[1,21] == 14
    assert e(feppml_receipt)[1,22] == 5
    assert e(feppml_receipt)[1,33] == 1
    mata: assert(all(st_matrix("e(V)") :< .))
    mata: assert(max(abs(st_matrix("e(V)") :- st_matrix("e(V)")')) <= 1e-10)
    if e(V)[1,1] > 0 {
        capture confirm scalar e(chi2)
        assert _rc == 0
        test x
        assert r(chi2) >= 0 & r(chi2) < .
        assert r(df) == 1
    }
    else {
        assert e(V)[1,1] == 0
        capture confirm scalar e(chi2)
        assert _rc == 111
        capture confirm scalar e(df_chi2)
        assert _rc == 111
        test x
        assert missing(r(chi2))
        assert r(df) == 0
    }
}

* Normalize only the VCE keyword; Stata variable identities remain
* case-sensitive throughout one- and multiway cluster publication.
generate long FirmID = c1
generate long RegionID = c2
`feppml_test_command' y c.x, separation(none) vce(CLUSTER FirmID)
assert `"`e(clustvar1)'"' == "FirmID"
assert `"`e(clustvar)'"' == "FirmID"
`feppml_test_command' y c.x, separation(none) ///
    vce(ClUsTeR FirmID RegionID)
assert `"`e(clustvar1)'"' == "FirmID"
assert `"`e(clustvar2)'"' == "RegionID"
assert `"`e(clustvar)'"' == "FirmID RegionID"

if !`feppml_hidden_wrapper' ///
    display as result "END_FIXTURE|feppml_weights_inference|rc=0"
