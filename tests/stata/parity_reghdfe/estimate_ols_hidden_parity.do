version 18.0
clear all
set more off

quietly findfile fereg.ado
local ado_path = subinstr(`"`r(fn)'"', "\", "/", .)
local suffix "/stata/fereg.ado"
assert substr(`"`ado_path'"', -strlen(`"`suffix'"'), .) == `"`suffix'"'
local root = substr(`"`ado_path'"', 1, strlen(`"`ado_path'"') - strlen(`"`suffix'"'))

* These are transport/eclass checks against the immutable licensed Stata 18
* receipt at benchmarks/reports/captured/darwin-arm64/stata18/ols/
* 20260821T030000Z-a5181f9. Internal scientific tolerances remain in Rust;
* the absolute receipt caps below only characterize pinned DEFAULT reghdfe.
import delimited using `"`root'/tests/fixtures/ols/ols_core.csv"', clear

_fereg_estimate_ols_test y c.x1 c.x2, residual(__frg_iid_resid)
assert e(N) == 24
assert e(df_m) == 2
assert e(df_a) == 1
assert e(df_r) == 21
assert `"`e(cmd)'"' == "_fereg_estimate_ols_test"
assert `"`e(model)'"' == "ols"
assert `"`e(depvar)'"' == "y"
assert `"`e(indepvars)'"' == "x1 x2"
assert `"`e(vce)'"' == "ols"
assert `"`e(fereg_action_protocol)'"' == "1.3"
assert `"`e(fereg_model_wire)'"' == "2.1"
assert `"`e(fereg_result_wire)'"' == "1.3"
assert e(fereg_receipt)[1,1] == 5354601
assert e(fereg_receipt)[1,4] == 1
assert e(fereg_receipt)[1,16] == 1
assert e(fereg_certificate)[24,2] == 1
assert e(fereg_certificate)[31,1] == 1
assert e(fereg_certificate)[31,2] == det(e(V))
assert e(fereg_certificate)[32,2] == 1
assert e(sample) == 1
assert !missing(__frg_iid_resid)
assert abs(e(b)[1,1] - +1.6094dc78ce848X+000) <= 3e-6
assert abs(e(b)[1,2] - -1.57252c6b41c29X-006) <= 3e-6
assert abs(e(b)[1,3] - +1.84d9face23e96X+002) <= 3e-6
assert abs(e(V)[1,1] - +1.1759580b65592X-009) <= 3e-6
assert abs(e(V)[2,2] - +1.83d2fd489555fX-00a) <= 3e-6
assert abs(e(V)[3,3] - +1.465e95b087a5cX-009) <= 3e-6
assert abs(e(rss) - +1.2a14530d58416X+000) <= 3e-6
assert abs(e(tss) - +1.9d325a41a745bX+005) <= 3e-6

_fereg_estimate_ols_test y c.x1 i.industry, absorb(firm) vce(robust)
assert e(N) == 24
assert e(df_m) == 3
assert e(df_a) == 8
assert e(df_r) == 13
assert `"`e(vce)'"' == "robust"
assert `"`e(vcetype)'"' == "Robust"
assert `"`e(absvars)'"' == "firm"
local robust_names : colfullnames e(b)
assert `"`robust_names'"' == "x1 1b.industry 2.industry 3.industry _cons"
assert e(fereg_status)[2,2] == 2
assert e(b)[1,2] == 0
assert e(V)[2,1] == 0 & e(V)[1,2] == 0
assert e(V)[2,2] == 0
assert rowsof(e(dof_table)) == 1 & colsof(e(dof_table)) == 5
local dof_rows : rowfullnames e(dof_table)
local dof_columns : colnames e(dof_table)
assert `"`dof_rows'"' == "1.firm"
assert `"`dof_columns'"' == "Categories Redundant Num Coefs Inexact? Nested?"
assert e(dof_table)[1,1] == 8
assert e(dof_table)[1,2] == 0
assert e(dof_table)[1,3] == 8
assert e(dof_table)[1,4] == 0
assert e(dof_table)[1,5] == 0
assert abs(e(V)[1,1] - +1.72b3a43070d30X-00b) <= 3e-6
assert abs(e(V)[1,3] - -1.aacc28464cca4X-00c) <= 3e-6
assert abs(e(V)[1,4] - -1.661d37ef6b7e8X-00a) <= 3e-6
assert abs(e(V)[5,5] - +1.31b4b891013a3X-009) <= 3e-6

* Full frozen multi_fe_interactions case: the ordinary factor interaction,
* absorbed categorical tuple, final-sample rebase, omission axis, and
* reference e(indepvars) contract must hold together in the hidden adapter.
_fereg_ols_hidden_numeric_v1 y c.x1 i.industry##i.year, ///
    absorb(worker firm#year)
assert e(N) == 8
assert e(df_m) == 1
assert e(df_a) == 6
assert e(df_r) == 1
assert e(N_hdfe) == 2
assert `"`e(absvars)'"' == "worker firm#year"
assert `"`e(indepvars)'"' == ///
    "x1 3bn.industry 2022bn.year 3bn.industry#2022bn.year _cons"
local interaction_names : colfullnames e(b)
assert `"`interaction_names'"' == ///
    "o.x1 2b.industry 3o.industry 2021b.year 2022o.year 2b.industry#2021b.year 2b.industry#2022o.year 3o.industry#2021b.year 3.industry#2022.year _cons"
assert e(fereg_status)[1,2] == 3
assert e(fereg_status)[2,2] == 2
assert e(fereg_status)[3,2] == 3
assert e(fereg_status)[4,2] == 2
assert e(fereg_status)[5,2] == 3
assert e(fereg_status)[6,2] == 2
assert e(fereg_status)[7,2] == 2
assert e(fereg_status)[8,2] == 2
assert e(fereg_status)[9,2] == 1
assert e(fereg_status)[10,2] == 5
assert abs(e(b)[1,9] - +1.ffffffdcf4f1eX-017) <= 1e-7
assert abs(e(b)[1,10] - +1.b999998000000X+002) <= 1e-7
assert abs(e(V)[9,9] - +1.47ae1eb851fc4X-005) <= 1e-6
assert abs(e(V)[9,10] - -1.47ae1eb851fc4X-007) <= 1e-6
assert abs(e(V)[10,10] - +1.47ae1eb851fc5X-008) <= 1e-6
local interaction_dof_rows : rowfullnames e(dof_table)
assert `"`interaction_dof_rows'"' == "1.worker 1.firm#1.year"
assert e(dof_table)[1,1] == 4 & e(dof_table)[1,2] == 0
assert e(dof_table)[1,3] == 4
assert e(dof_table)[2,1] == 4 & e(dof_table)[2,2] == 2
assert e(dof_table)[2,3] == 2

_fereg_estimate_ols_test y c.x1, absorb(worker firm) cluster(cluster_a)
assert e(df_r) == 7
local clustered_dof_rows : rowfullnames e(dof_table)
assert `"`clustered_dof_rows'"' == "1.worker 1.firm"
assert e(N_clust) == 8 & e(N_clust1) == 8
assert e(fereg_certificate)[25,2] == 1
assert e(fereg_certificate)[26,2] == 0
assert e(fereg_certificate)[27,2] == 0
assert e(fereg_certificate)[29,2] == 1
assert det(e(V)) >= -1e-16
assert abs(e(V)[1,1] - +1.55d7b24d54e16X-006) <= 1e-7
assert abs(e(V)[1,2] - -1.8ed0faa7131f0X-008) <= 1e-7
assert abs(e(V)[2,2] - +1.d1492463b1f98X-00a) <= 1e-7

_fereg_estimate_ols_test y c.x1, absorb(worker firm) ///
    cluster(cluster_a cluster_b)
assert e(df_r) == 3
assert e(df_a) == 0
assert e(N_clustervars) == 2
assert e(N_clust1) == 8 & e(N_clust2) == 4 & e(N_clust) == 4
assert e(fereg_subset)[1,1] == 1 & e(fereg_subset)[1,3] == 1
assert e(fereg_subset)[2,1] == 2 & e(fereg_subset)[2,3] == 1
assert e(fereg_subset)[3,1] == 3 & e(fereg_subset)[3,3] == -1
assert e(fereg_certificate)[25,2] == 4
assert e(fereg_certificate)[26,2] == 1
assert e(fereg_certificate)[27,2] == 1
assert e(fereg_certificate)[28,2] < 0
assert e(fereg_certificate)[29,2] == 0
assert e(V)[1,1] == +1.0202f53f9fd93X-005
assert e(V)[1,2] == -1.2c7939cef68e3X-007
assert e(V)[2,1] == -1.2c7939cef68e3X-007
assert e(V)[2,2] == +1.5dded0e415712X-009
assert det(e(V)) == -1.b7ee825ec19fdX-01b
assert e(fereg_certificate)[31,2] == -1.b7ee825ec19fdX-01b
assert abs(e(V)[1,1] - +1.0202f4787935bX-005) <= 2e-6
assert abs(e(V)[1,2] - -1.2c88da1cac911X-007) <= 2e-6
assert abs(e(V)[2,2] - +1.5e0658149d738X-009) <= 2e-6

_fereg_estimate_ols_test y c.x1, absorb(worker firm) ///
    cluster(cluster_a cluster_b cluster_c)
assert e(df_r) == 2
assert e(df_a) == 0
assert e(N_clustervars) == 3
assert e(N_clust1) == 8 & e(N_clust2) == 4 & e(N_clust3) == 3
assert rowsof(e(fereg_subset)) == 7
assert e(fereg_certificate)[25,2] == 4
assert e(fereg_certificate)[26,2] == 1
assert e(fereg_certificate)[27,2] == 1
assert e(fereg_certificate)[28,2] < 0
assert e(fereg_certificate)[29,2] == 0
assert e(V)[1,1] == +1.337c1601e503aX-006
assert e(V)[1,2] == -1.6a2d73734b664X-008
assert e(V)[2,1] == -1.6a2d73734b664X-008
assert e(V)[2,2] == +1.aa6d2d3865d7aX-00a
assert det(e(V)) == -1.a79e7644cecccX-01b
assert e(fereg_certificate)[31,2] == -1.a79e7644cecccX-01b
assert abs(e(V)[1,1] - +1.337c143e3831eX-006) <= 4e-6
assert abs(e(V)[1,2] - -1.6a5e0063700aaX-008) <= 4e-6
assert abs(e(V)[2,2] - +1.aae99ff9c51f2X-00a) <= 4e-6

_fereg_estimate_ols_test y c.x1, absorb(worker firm) ///
    cluster(cluster_a#cluster_b)
assert e(df_a) == 18 & e(df_r) == 5
assert e(N_clust) == 16 & e(N_clustervars) == 1
assert `"`e(clustvar)'"' == "cluster_a#cluster_b"
assert e(fereg_cluster)[1,2] == 2
assert e(fereg_cluster)[1,3] == 16
assert e(fereg_cluster)[1,4] == 1
assert e(fereg_certificate)[25,2] == 1
assert e(fereg_certificate)[29,2] == 1
assert det(e(V)) >= -1e-16
assert abs(e(V)[1,1] - +1.c714def42ee8cX-006) <= 1e-7
assert abs(e(V)[1,2] - -1.0976d76bf8296X-007) <= 1e-7
assert abs(e(V)[2,2] - +1.547154bc25ea1X-009) <= 1e-7

import delimited using `"`root'/tests/fixtures/ols/ols_weights.csv"', clear
_fereg_estimate_ols_test y c.x, absorb(group) aweight(w_valid)
assert e(N) == 12 & e(df_r) == 5
assert abs(e(sumweights) - 25.8) <= 1e-6
assert `"`e(wtype)'"' == "aweight" & `"`e(wexp)'"' == "= w_valid"
assert abs(e(b)[1,1] - +1.ec1c26732c450X+000) <= 3e-6
assert abs(e(V)[1,1] - +1.599e09a857cddX-005) <= 3e-6
assert abs(e(V)[2,2] - +1.3ee1529cf1611X-005) <= 3e-6

_fereg_estimate_ols_test y c.x, absorb(group) fweight(fw_valid)
assert e(N) == 26 & e(df_r) == 19 & e(sumweights) == 26
assert `"`e(wtype)'"' == "fweight" & `"`e(wexp)'"' == "= fw_valid"
assert abs(e(b)[1,1] - +1.071c74a8cc936X+001) <= 3e-6
assert abs(e(V)[1,1] - +1.37267e35e5f40X-007) <= 3e-6
assert abs(e(V)[2,2] - +1.85bac823e9064X-009) <= 3e-6

_fereg_estimate_ols_test y c.x, absorb(group) pweight(pw_valid)
assert e(N) == 12 & e(df_r) == 5
assert abs(e(sumweights) - 12.4) <= 1e-6
assert `"`e(wtype)'"' == "pweight" & `"`e(wexp)'"' == "= pw_valid"
assert `"`e(vce)'"' == "robust" & `"`e(vcetype)'"' == "Robust"
assert abs(e(b)[1,1] - +1.0a633d2c1dd4cX+001) <= 3e-6
assert abs(e(V)[1,1] - +1.464d18a4af078X-005) <= 3e-6
assert abs(e(V)[2,2] - +1.6ca0cbb6a2a2aX-007) <= 3e-6

generate double __frg_aw_negative = 1
replace __frg_aw_negative = -1 in 1
capture noisily _fereg_estimate_ols_test y c.x, absorb(group) aweight(__frg_aw_negative)
assert _rc == 402

generate double __frg_fw_fractional = fw_valid
replace __frg_fw_fractional = 1.5 in 1
capture noisily _fereg_estimate_ols_test y c.x, absorb(group) fweight(__frg_fw_fractional)
assert _rc == 401

capture noisily _fereg_estimate_ols_test y c.x if 0, absorb(group)
assert _rc == 2001

di as result "END_FIXTURE|estimate_ols_hidden_parity|rc=0"
