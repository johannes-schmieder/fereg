version 18.0
local feppml_test_command `"$FEREG_PPML_TEST_COMMAND"'
local feppml_hidden_wrapper = strtrim(`"$FEREG_PPML_TEST_COMMAND"') != ""
if strtrim(`"`feppml_test_command'"') == "" local feppml_test_command "feppml"
clear all
set more off

* Every documented prediction and residual is checked from its defining
* observation-level identity, including the y=0 deviance branch.
set obs 18
generate long obs_id = _n
generate double x = mod(5 * _n, 13) / 4 - 1.5
generate double y = mod(7 * _n, 9)
replace y = y + 0.5 if mod(_n, 4) == 0

`feppml_test_command' y c.x, separation(none) ///
    residuals(__response) residualtype(response) ///
    xb(__xb) d(__absorbed) eta(__eta) mu(__mu)
assert e(sample) == 1
assert abs(__eta - __xb - __absorbed) <= 1e-10
assert abs(__absorbed) <= 1e-10
assert abs(__mu - exp(__eta)) <= 1e-10 * max(__mu, 1)
assert abs(__response - (y - __mu)) <= 1e-10 * max(__mu, 1)

predict double __predict_default
predict double __predict_mu, mu
predict double __predict_xb, xb
predict double __predict_d, d
predict double __predict_eta, eta
predict double __predict_response, response
predict double __predict_score, score
predict double __predict_pearson, pearson
predict double __predict_working, working
predict double __predict_deviance, deviance
predict double __predict_anscombe, anscombe

assert abs(__predict_default - __mu) <= 1e-12
assert abs(__predict_mu - __mu) <= 1e-12
assert abs(__predict_xb - __xb) <= 1e-12
assert abs(__predict_d - __absorbed) <= 1e-12
assert abs(__predict_eta - __eta) <= 1e-12
assert abs(__predict_response - (y - __mu)) <= 1e-10 * max(__mu, 1)
assert abs(__predict_score - (y - __mu)) <= 1e-10 * max(__mu, 1)
assert abs(__predict_pearson - (y - __mu) / sqrt(__mu)) <= 1e-10
assert abs(__predict_working - (y - __mu) / __mu) <= 1e-10
assert abs(__predict_anscombe - ///
    1.5 * (y^(2/3) - __mu^(2/3)) / __mu^(1/6)) <= 1e-10
assert abs(__predict_deviance - 2 * __mu) <= 1e-10 if y == 0
assert abs(__predict_deviance - ///
    2 * (__mu - y + y * ln(y / __mu))) <= 1e-10 if y > 0

capture noisily predict double __invalid_prediction, xb mu
assert _rc == 198
confirm new variable __invalid_prediction

* residuals() defaults to response; every explicit residualtype() uses the
* same public formulas as predict.
`feppml_test_command' y c.x, separation(none) residuals(__default_response) mu(__default_mu)
assert abs(__default_response - (y - __default_mu)) <= ///
    1e-10 * max(__default_mu, 1)
drop __default_response __default_mu

foreach residual_type in score pearson working deviance anscombe {
    `feppml_test_command' y c.x, separation(none) residuals(__typed_residual) ///
        residualtype(`residual_type') mu(__typed_mu)
    if `"`residual_type'"' == "score" {
        assert abs(__typed_residual - (y - __typed_mu)) <= ///
            1e-10 * max(__typed_mu, 1)
    }
    if `"`residual_type'"' == "pearson" {
        assert abs(__typed_residual - (y - __typed_mu) / sqrt(__typed_mu)) <= 1e-10
    }
    if `"`residual_type'"' == "working" {
        assert abs(__typed_residual - (y - __typed_mu) / __typed_mu) <= 1e-10
    }
    if `"`residual_type'"' == "deviance" {
        assert abs(__typed_residual - 2 * __typed_mu) <= 1e-10 if y == 0
        assert abs(__typed_residual - ///
            2 * (__typed_mu - y + y * ln(y / __typed_mu))) <= 1e-10 if y > 0
    }
    if `"`residual_type'"' == "anscombe" {
        assert abs(__typed_residual - 1.5 * ///
            (y^(2/3) - __typed_mu^(2/3)) / __typed_mu^(1/6)) <= 1e-10
    }
    drop __typed_residual __typed_mu
}

* A zero-only FE level is certified by the FE detector, removed from the
* final sample, and assigned its stable reason code.
clear
set obs 24
generate long obs_id = _n
generate byte fe = ceil(_n / 6)
generate double x = mod(_n - 1, 3) - 1
generate double off = 0.02 * mod(_n, 2)
generate double y = 0.5 + mod(5 * _n, 7)
replace y = 0 if fe == 4

`feppml_test_command' y c.x, absorb(__saved_fe=fe) offset(off) ///
    residuals(__saved_response) residualtype(response) ///
    xb(__saved_xb) d(__saved_d) eta(__saved_eta) mu(__saved_mu) ///
    dropreason(__saved_drop)
assert e(converged) == 1
assert e(N_full) == 24
assert e(N_physical) == 18
assert e(separated) == 6
assert e(sample) == (fe < 4)
assert __saved_drop == 0 if fe < 4
assert __saved_drop == 9 if fe == 4
assert missing(__saved_fe) & missing(__saved_response) & ///
    missing(__saved_xb) & missing(__saved_d) & ///
    missing(__saved_eta) & missing(__saved_mu) if fe == 4
assert abs(__saved_eta - __saved_xb - __saved_d) <= 1e-9 if e(sample)
assert abs(__saved_d - __saved_fe) <= 1e-8 if e(sample)
assert abs(__saved_mu - exp(__saved_eta)) <= ///
    1e-9 * max(__saved_mu, 1) if e(sample)
assert abs(__saved_response - (y - __saved_mu)) <= ///
    1e-9 * max(__saved_mu, 1) if e(sample)
bysort fe (obs_id): assert abs(__saved_fe - __saved_fe[1]) <= 1e-10 if e(sample)
* The structural intercept normalization is counted once in finite rank;
* three retained one-way FE levels therefore contribute L - 1 absorbed rank.
assert e(rank) == 2
assert e(feppml_status)[2,2] == 7
assert e(b)[1,2] == 0
assert e(df_a) == 2
assert e(df_resid) == e(N_effective) - e(rank) - e(df_a)
assert colsof(e(feppml_separation)) == 14
assert rowsof(e(feppml_separation)) == e(feppml_receipt)[1,17]
mata: assert(any(st_matrix("e(feppml_separation)")[.,1] :== 2))
mata: assert(any((st_matrix("e(feppml_separation)")[.,1] :== 2) :& (st_matrix("e(feppml_separation)")[.,6] :== 6)))
assert e(feppml_receipt)[1,16] == 6
assert e(feppml_receipt)[1,20] == 16
assert e(feppml_receipt)[1,21] == 14
assert e(feppml_receipt)[1,22] == 5
assert e(feppml_receipt)[1,33] == 1

* Asking for normalized FE channels may not change the scientific estimate,
* reconstructed predictor, fitted mean, or final sample.
tempname saved_b saved_V
matrix `saved_b' = e(b)
matrix `saved_V' = e(V)
scalar __saved_deviance = e(deviance)
generate byte __saved_sample_copy = e(sample)
clonevar __saved_eta_copy = __saved_eta
clonevar __saved_mu_copy = __saved_mu

drop __saved_fe __saved_response __saved_xb __saved_d __saved_eta ///
    __saved_mu __saved_drop
`feppml_test_command' y c.x, absorb(fe) offset(off) ///
    residuals(__plain_response) residualtype(response) ///
    xb(__plain_xb) d(__plain_d) eta(__plain_eta) mu(__plain_mu) ///
    dropreason(__plain_drop)
assert mreldif(e(b), `saved_b') <= 1e-9
assert mreldif(e(V), `saved_V') <= 1e-8
assert reldif(e(deviance), scalar(__saved_deviance)) <= 1e-10
assert e(sample) == __saved_sample_copy
assert abs(__plain_eta - __saved_eta_copy) <= 1e-8 if e(sample)
assert abs(__plain_mu - __saved_mu_copy) <= ///
    1e-8 * max(__plain_mu, 1) if e(sample)
assert __plain_drop == 9 if fe == 4

predict double __post_mu, mu
predict double __post_eta, eta
predict double __post_xb, xb
predict double __post_d, d
assert abs(__post_mu - __plain_mu) <= 1e-12 if e(sample)
assert abs(__post_eta - __plain_eta) <= 1e-12 if e(sample)
assert abs(__post_xb - __plain_xb) <= 1e-12 if e(sample)
assert abs(__post_d - __plain_d) <= 1e-10 if e(sample)
assert missing(__post_mu) & missing(__post_eta) & missing(__post_xb) & ///
    missing(__post_d) if !e(sample)

* Output-name validation and numerical nonconvergence are transactional.
tempname accepted_b accepted_V
matrix `accepted_b' = e(b)
matrix `accepted_V' = e(V)
generate byte __accepted_sample = e(sample)

capture noisily `feppml_test_command' y c.x, absorb(fe) ///
    residuals(__duplicate_output) mu(__duplicate_output)
assert _rc == 110
confirm new variable __duplicate_output
assert e(sample) == __accepted_sample
assert mreldif(e(b), `accepted_b') == 0
assert mreldif(e(V), `accepted_V') == 0

clear
set obs 80
generate long obs_id = _n
generate double x = (_n - 40) / 8
generate double y = ceil(exp(0.55 * x + 0.15 * sin(_n)))
quietly regress y x
tempname before_b before_V
matrix `before_b' = e(b)
matrix `before_V' = e(V)
generate byte __before_sample = e(sample)
capture noisily `feppml_test_command' y c.x, separation(none) iterate(1) tolerance(1e-14) ///
    residuals(__nonconverged_resid) eta(__nonconverged_eta) ///
    mu(__nonconverged_mu) dropreason(__nonconverged_drop)
assert _rc == 430
confirm new variable __nonconverged_resid
confirm new variable __nonconverged_eta
confirm new variable __nonconverged_mu
confirm new variable __nonconverged_drop
assert `"`e(cmd)'"' == "regress"
assert e(sample) == __before_sample
assert mreldif(e(b), `before_b') == 0
assert mreldif(e(V), `before_V') == 0

if !`feppml_hidden_wrapper' ///
    display as result "END_FIXTURE|feppml_separation_outputs|rc=0"
