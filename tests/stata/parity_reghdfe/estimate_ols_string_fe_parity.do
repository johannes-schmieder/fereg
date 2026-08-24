version 18.0
clear all
set more off

quietly findfile fereg.ado
local ado_path = subinstr(`"`r(fn)'"', "\", "/", .)
local suffix "/stata/fereg.ado"
assert substr(`"`ado_path'"', -strlen(`"`suffix'"'), .) == `"`suffix'"'
local root = substr(`"`ado_path'"', 1, strlen(`"`ado_path'"') - strlen(`"`suffix'"'))

import delimited using `"`root'/tests/fixtures/ols/ols_core.csv"', clear
_fereg_ols_hidden_numeric_v1 y c.x1, ///
    absorb(firm_s year) residual(__frg_string_resid)

assert e(N) == 24
assert e(N_full) == 24
assert e(df_m) == 1
assert e(df_a) == 11
assert e(df_a_initial) == 12
assert e(df_a_redundant) == 1
assert e(df_r) == 12
assert e(N_hdfe) == 2
assert e(N_hdfe_extended) == 2
assert e(num_singletons) == 0
assert e(rank) == 1
assert e(report_constant) == 1
assert e(converged) == 1
assert `"`e(absvars)'"' == "firm_s year"
assert `"`e(vce)'"' == "ols"
assert e(sample) == 1
assert !missing(__frg_string_resid)

local coefficient_names : colfullnames e(b)
assert `"`coefficient_names'"' == "x1 _cons"
assert e(fereg_status)[1,2] == 1
assert e(fereg_status)[2,2] == 5
assert abs(e(b)[1,1] - +1.62c64904a527dX+000) <= 1e-10
assert abs(e(b)[1,2] - +1.84cc354cfe9fcX+002) <= 1e-10
assert abs(e(V)[1,1] - +1.e14ebd23cab23X-00c) <= 1e-10
assert abs(e(V)[1,2] - -1.18c343aa363d4X-00d) <= 1e-10
assert abs(e(V)[2,1] - -1.18c343aa363d4X-00d) <= 1e-10
assert abs(e(V)[2,2] - +1.d120be44c7154X-00c) <= 1e-10

assert rowsof(e(dof_table)) == 2
assert colsof(e(dof_table)) == 5
local dof_rows : rowfullnames e(dof_table)
assert `"`dof_rows'"' == "1.firm_s 1.year"
assert e(dof_table)[1,1] == 8
assert e(dof_table)[1,2] == 0
assert e(dof_table)[1,3] == 8
assert e(dof_table)[1,4] == 0
assert e(dof_table)[1,5] == 0
assert e(dof_table)[2,1] == 4
assert e(dof_table)[2,2] == 1
assert e(dof_table)[2,3] == 3
assert e(dof_table)[2,4] == 0
assert e(dof_table)[2,5] == 0
assert e(fereg_fe)[1,4] == 8
assert e(fereg_fe)[2,4] == 4
assert e(fereg_certificate)[1,2] == 1
assert e(fereg_certificate)[2,2] == 1
assert e(fereg_certificate)[3,2] == 1
assert e(fereg_certificate)[24,2] == 1
assert e(fereg_certificate)[31,2] > 0
assert e(fereg_certificate)[31,2] == det(e(V))
assert e(fereg_certificate)[32,2] == 1

tempname string_b string_v
matrix `string_b' = e(b)
matrix `string_v' = e(V)
generate double __frg_string_resid_copy = __frg_string_resid

generate strL firm_l = firm_s
capture noisily _fereg_ols_hidden_numeric_v1 y c.x1, absorb(firm_l year)
assert _rc == 498
assert mreldif(e(b), `string_b') == 0
assert mreldif(e(V), `string_v') == 0
assert `"`e(absvars)'"' == "firm_s year"

_fereg_ols_hidden_numeric_v1 y c.x1, ///
    absorb(firm year) residual(__frg_numeric_resid)
assert mreldif(e(b), `string_b') <= 1e-12
assert mreldif(e(V), `string_v') <= 1e-12
assert abs(__frg_numeric_resid - __frg_string_resid_copy) <= 1e-10

capture noisily fereg y c.x1, absorb(firm_s year)
assert _rc == 0
assert `"`e(cmd)'"' == "fereg"
assert e(N) == 24

di as result "END_FIXTURE|estimate_ols_string_fe_parity|rc=0"
