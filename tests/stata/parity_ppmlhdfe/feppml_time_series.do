version 18.0
local feppml_test_command `"$FEREG_PPML_TEST_COMMAND"'
local feppml_hidden_wrapper = strtrim(`"$FEREG_PPML_TEST_COMMAND"') != ""
if strtrim(`"`feppml_test_command'"') == "" local feppml_test_command "feppml"
clear all
set more off

* Stata owns time-series indexing.  Build three panels with two interior gaps,
* then materialize the independent reference columns before applying if/in.
set obs 36
generate long obs_id = _n
generate byte panel = ceil(_n / 12)
generate byte within_panel = 1 + mod(_n - 1, 12)
generate int period = within_panel + (within_panel >= 5) + (within_panel >= 9)
generate double x = 0.3 * panel + mod(7 * _n, 19) / 9
generate double x_original = x
generate double y = 0.6 + mod(11 * _n, 17) / 4
generate byte candidate = within_panel != 6
xtset panel period

generate double __manual_lag = L.x
generate double __manual_lead = F.x
generate double __manual_difference = D.x
assert missing(__manual_lead) if within_panel == 4
assert missing(__manual_lag) if within_panel == 5
assert missing(__manual_difference) if within_panel == 5

generate byte __expected_sample = candidate & inrange(obs_id, 2, 35) & ///
    !missing(y, __manual_lag, __manual_lead, __manual_difference)

`feppml_test_command' y L.x F.x D.x if candidate in 2/35, ///
    separation(none) keepsingletons memory(fast)
local ts_names : colfullnames e(b)
assert `"`ts_names'"' == "L.x F.x D.x _cons"
assert e(sample) == __expected_sample
* Row two uses row one's lag although row one is outside in 2/35.  Within-panel
* row seven uses a lag from row six even though row six fails if candidate.
assert e(sample) if obs_id == 2
assert e(sample) if within_panel == 7 & inrange(obs_id, 2, 35)
assert x == x_original
tempname ts_b ts_V
matrix `ts_b' = e(b)
matrix `ts_V' = e(V)
scalar __ts_deviance = e(deviance)
scalar __ts_ll = e(ll)
scalar __ts_ll0 = e(ll_0)
scalar __ts_N = e(N)
generate byte __ts_sample = e(sample)
predict double __ts_predict_xb if candidate in 2/35, xb
assert __ts_predict_xb < . if __ts_sample
assert missing(__ts_predict_xb) if !__ts_sample
generate double __ts_manual_xb = _b[_cons] + _b[L.x] * L.x + ///
    _b[F.x] * F.x + _b[D.x] * D.x if __ts_sample
assert abs(__ts_predict_xb - __ts_manual_xb) <= ///
    1e-10 * max(1, abs(__ts_manual_xb)) if __ts_sample

`feppml_test_command' y c.__manual_lag c.__manual_lead ///
    c.__manual_difference if candidate in 2/35, ///
    separation(none) keepsingletons memory(fast)
assert e(sample) == __ts_sample
assert e(N) == __ts_N
assert mreldif(e(b), `ts_b') <= 1e-11
assert mreldif(e(V), `ts_V') <= 1e-9
assert reldif(e(deviance), __ts_deviance) <= 1e-11
assert reldif(e(ll), __ts_ll) <= 1e-11
assert reldif(e(ll_0), __ts_ll0) <= 1e-11
assert x == x_original

if !`feppml_hidden_wrapper' ///
    display as result "END_FIXTURE|feppml_time_series|rc=0"
