version 18.0
clear all
set more off

quietly findfile fereg.ado
local ado_path = subinstr(`"`r(fn)'"', "\", "/", .)
local suffix "/stata/fereg.ado"
assert substr(`"`ado_path'"', -strlen(`"`suffix'"'), .) == `"`suffix'"'
local root = substr(`"`ado_path'"', 1, strlen(`"`ado_path'"') - strlen(`"`suffix'"'))

import delimited using `"`root'/tests/fixtures/ols/ols_ts.csv"', clear
xtset panel year
_fereg_ols_hidden_numeric_v1 y L.x, absorb(panel)

assert e(N) == 12
assert e(N_full) == 12
assert e(df_m) == 1
assert e(df_a) == 3
assert e(df_r) == 8
assert e(rank) == 1
assert e(num_singletons) == 0
assert `"`e(indepvars)'"' == "L.x _cons"
assert `"`e(absvars)'"' == "panel"
assert e(sample) == inlist(obs_id, 2, 3, 4, 6, 8, 9, 10, 12, 14, 15, 16, 18)
assert abs(e(b)[1,1] - +1.270f02a5101a0X+000) <= 1e-10
assert abs(e(b)[1,2] - +1.556267538f239X+001) <= 1e-10
assert abs(e(V)[1,1] - +1.ba1098b4b5749X-006) <= 1e-10
assert abs(e(V)[1,2] - -1.6905058614c09X-006) <= 1e-10
assert abs(e(V)[2,1] - -1.6905058614c09X-006) <= 1e-10
assert abs(e(V)[2,2] - +1.7a4d99cd796b5X-005) <= 1e-10
tempname lag_b lag_v
matrix `lag_b' = e(b)
matrix `lag_v' = e(V)
generate byte __frg_lag_sample = e(sample)

generate double __frg_manual_lag = L.x
_fereg_ols_hidden_numeric_v1 y c.__frg_manual_lag, absorb(panel)
assert mreldif(e(b), `lag_b') <= 1e-12
assert mreldif(e(V), `lag_v') <= 1e-12
assert e(sample) == __frg_lag_sample

_fereg_ols_hidden_numeric_v1 y F.x D.x

assert e(N) == 6
assert e(N_full) == 6
assert e(df_m) == 2
assert e(df_a) == 1
assert e(df_r) == 3
assert e(rank) == 2
assert `"`e(indepvars)'"' == "F.x D.x _cons"
assert e(sample) == inlist(obs_id, 2, 3, 8, 9, 14, 15)
assert abs(e(b)[1,1] - +1.8ad016f50c0b7X-001) <= 1e-10
assert abs(e(b)[1,2] - +1.30836ede4a100X-001) <= 1e-10
assert abs(e(b)[1,3] - +1.4a29735435d3eX+000) <= 1e-10
assert abs(e(V)[1,1] - +1.34338aa04cc7bX-001) <= 1e-10
assert abs(e(V)[1,2] - +1.3ca52a93d8c3bX-002) <= 1e-10
assert abs(e(V)[1,3] - -1.c16cd24d58b94X-001) <= 1e-10
assert abs(e(V)[2,1] - +1.3ca52a93d8c3bX-002) <= 1e-10
assert abs(e(V)[2,2] - +1.f013d2eb5c228X-002) <= 1e-10
assert abs(e(V)[2,3] - -1.4ae4eacc24d88X-001) <= 1e-10
assert abs(e(V)[3,1] - -1.c16cd24d58b94X-001) <= 1e-10
assert abs(e(V)[3,2] - -1.4ae4eacc24d88X-001) <= 1e-10
assert abs(e(V)[3,3] - +1.79f87704f1cd4X+000) <= 1e-10
tempname lead_diff_b lead_diff_v
matrix `lead_diff_b' = e(b)
matrix `lead_diff_v' = e(V)
generate byte __frg_lead_diff_sample = e(sample)

generate double __frg_manual_lead = F.x
generate double __frg_manual_difference = D.x
_fereg_ols_hidden_numeric_v1 y ///
    c.__frg_manual_lead c.__frg_manual_difference
assert mreldif(e(b), `lead_diff_b') <= 1e-12
assert mreldif(e(V), `lead_diff_v') <= 1e-12
assert e(sample) == __frg_lead_diff_sample

capture noisily fereg y L.x, absorb(panel)
assert _rc == 0
assert `"`e(cmd)'"' == "fereg"
assert e(N) == 12

di as result "END_FIXTURE|estimate_ols_time_series_parity|rc=0"
