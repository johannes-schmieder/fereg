version 18.0
clear all
set more off

quietly findfile stfixest.ado
local ado_path = subinstr(`"`r(fn)'"', "\", "/", .)
local suffix "/stata/stfixest.ado"
assert substr(`"`ado_path'"', -strlen(`"`suffix'"'), .) == `"`suffix'"'
local root = substr(`"`ado_path'"', 1, strlen(`"`ado_path'"') - strlen(`"`suffix'"'))

* These are transport/eclass checks against the immutable licensed Stata 18
* receipt at benchmarks/reports/captured/darwin-arm64/stata18/ols/
* 20260821T030000Z-a5181f9. Internal scientific tolerances remain in Rust;
* the absolute receipt caps below only characterize pinned DEFAULT reghdfe.
import delimited using `"`root'/tests/fixtures/ols/ols_core.csv"', clear

_stfixest_estimate_ols_test y c.x1 c.x2, residual(__stfx_iid_resid)
assert e(N) == 24
assert e(df_m) == 2
assert e(df_a) == 1
assert e(df_r) == 21
assert `"`e(cmd)'"' == "_stfixest_estimate_ols_test"
assert `"`e(model)'"' == "ols"
assert `"`e(depvar)'"' == "y"
assert `"`e(indepvars)'"' == "x1 x2"
assert `"`e(vce)'"' == "ols"
assert `"`e(stfixest_action_protocol)'"' == "1.1"
assert `"`e(stfixest_model_wire)'"' == "2.1"
assert `"`e(stfixest_result_wire)'"' == "1.0"
assert e(stfixest_receipt)[1,1] == 5354601
assert e(stfixest_receipt)[1,4] == 1
assert e(stfixest_receipt)[1,16] == 1
assert e(stfixest_certificate)[24,2] == 1
assert e(sample) == 1
assert !missing(__stfx_iid_resid)
assert abs(e(b)[1,1] - +1.6094dc78ce848X+000) <= 3e-6
assert abs(e(b)[1,2] - -1.57252c6b41c29X-006) <= 3e-6
assert abs(e(b)[1,3] - +1.84d9face23e96X+002) <= 3e-6
assert abs(e(V)[1,1] - +1.1759580b65592X-009) <= 3e-6
assert abs(e(V)[2,2] - +1.83d2fd489555fX-00a) <= 3e-6
assert abs(e(V)[3,3] - +1.465e95b087a5cX-009) <= 3e-6
assert abs(e(rss) - +1.2a14530d58416X+000) <= 3e-6
assert abs(e(tss) - +1.9d325a41a745bX+005) <= 3e-6

_stfixest_estimate_ols_test y c.x1 i.industry, absorb(firm) vce(robust)
assert e(N) == 24
assert e(df_m) == 3
assert e(df_a) == 8
assert e(df_r) == 13
assert `"`e(vce)'"' == "robust"
assert `"`e(vcetype)'"' == "Robust"
assert `"`e(absvars)'"' == "firm"
local robust_names : colfullnames e(b)
assert `"`robust_names'"' == "x1 1b.industry 2.industry 3.industry _cons"
assert e(stfixest_status)[2,2] == 2
assert e(b)[1,2] == 0
assert e(V)[2,1] == 0 & e(V)[1,2] == 0
assert e(V)[2,2] == 0
assert rowsof(e(dof_table)) == 1 & colsof(e(dof_table)) == 5
local dof_rows : rowfullnames e(dof_table)
local dof_columns : colnames e(dof_table)
assert `"`dof_rows'"' == "firm"
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

_stfixest_estimate_ols_test y c.x1, absorb(worker firm) cluster(cluster_a)
assert e(df_r) == 7
assert e(N_clust) == 8 & e(N_clust1) == 8
assert e(stfixest_certificate)[25,2] == 1
assert e(stfixest_certificate)[26,2] == 0
assert e(stfixest_certificate)[27,2] == 0
assert e(stfixest_certificate)[29,2] == 1
assert det(e(V)) >= -1e-16
assert abs(e(V)[1,1] - +1.55d7b24d54e16X-006) <= 1e-7
assert abs(e(V)[1,2] - -1.8ed0faa7131f0X-008) <= 1e-7
assert abs(e(V)[2,2] - +1.d1492463b1f98X-00a) <= 1e-7

_stfixest_estimate_ols_test y c.x1, absorb(worker firm) ///
    cluster(cluster_a cluster_b)
assert e(df_r) == 3
assert e(df_a) == 0
assert e(N_clustervars) == 2
assert e(N_clust1) == 8 & e(N_clust2) == 4 & e(N_clust) == 4
assert e(stfixest_subset)[1,1] == 1 & e(stfixest_subset)[1,3] == 1
assert e(stfixest_subset)[2,1] == 2 & e(stfixest_subset)[2,3] == 1
assert e(stfixest_subset)[3,1] == 3 & e(stfixest_subset)[3,3] == -1
assert e(stfixest_certificate)[25,2] == 4
assert e(stfixest_certificate)[26,2] == 1
assert e(stfixest_certificate)[27,2] == 1
assert e(stfixest_certificate)[28,2] < 0
assert e(stfixest_certificate)[29,2] == 0
assert e(V)[1,1] == +1.0202f479f358fX-005
assert e(V)[1,2] == -1.2c7938ea4f98dX-007
assert e(V)[2,1] == -1.2c7938ea4f98dX-007
assert e(V)[2,2] == +1.5ddecfdb9c14bX-009
assert det(e(V)) == -1.b7ee8115e0f3cX-01b
assert abs(e(V)[1,1] - +1.0202f4787935bX-005) <= 2e-6
assert abs(e(V)[1,2] - -1.2c88da1cac911X-007) <= 2e-6
assert abs(e(V)[2,2] - +1.5e0658149d738X-009) <= 2e-6

_stfixest_estimate_ols_test y c.x1, absorb(worker firm) ///
    cluster(cluster_a cluster_b cluster_c)
assert e(df_r) == 2
assert e(df_a) == 0
assert e(N_clustervars) == 3
assert e(N_clust1) == 8 & e(N_clust2) == 4 & e(N_clust3) == 3
assert rowsof(e(stfixest_subset)) == 7
assert e(stfixest_certificate)[25,2] == 4
assert e(stfixest_certificate)[26,2] == 1
assert e(stfixest_certificate)[27,2] == 1
assert e(stfixest_certificate)[28,2] < 0
assert e(stfixest_certificate)[29,2] == 0
assert e(V)[1,1] == +1.337c145ab7e50X-006
assert e(V)[1,2] == -1.6a2d71c410628X-008
assert e(V)[2,1] == -1.6a2d71c410628X-008
assert e(V)[2,2] == +1.aa6d2b8b31611X-00a
assert det(e(V)) == -1.a79e7840b02b9X-01b
assert abs(e(V)[1,1] - +1.337c143e3831eX-006) <= 4e-6
assert abs(e(V)[1,2] - -1.6a5e0063700aaX-008) <= 4e-6
assert abs(e(V)[2,2] - +1.aae99ff9c51f2X-00a) <= 4e-6

_stfixest_estimate_ols_test y c.x1, absorb(worker firm) ///
    cluster(cluster_a#cluster_b)
assert e(df_a) == 18 & e(df_r) == 5
assert e(N_clust) == 16 & e(N_clustervars) == 1
assert `"`e(clustvar)'"' == "cluster_a#cluster_b"
assert e(stfixest_cluster)[1,2] == 2
assert e(stfixest_cluster)[1,3] == 16
assert e(stfixest_cluster)[1,4] == 1
assert e(stfixest_certificate)[25,2] == 1
assert e(stfixest_certificate)[29,2] == 1
assert det(e(V)) >= -1e-16
assert abs(e(V)[1,1] - +1.c714def42ee8cX-006) <= 1e-7
assert abs(e(V)[1,2] - -1.0976d76bf8296X-007) <= 1e-7
assert abs(e(V)[2,2] - +1.547154bc25ea1X-009) <= 1e-7

import delimited using `"`root'/tests/fixtures/ols/ols_weights.csv"', clear
_stfixest_estimate_ols_test y c.x, absorb(group) aweight(w_valid)
assert e(N) == 12 & e(df_r) == 5
assert abs(e(sumweights) - 25.8) <= 1e-6
assert `"`e(wtype)'"' == "aweight" & `"`e(wexp)'"' == "= w_valid"
assert abs(e(b)[1,1] - +1.ec1c26732c450X+000) <= 3e-6
assert abs(e(V)[1,1] - +1.599e09a857cddX-005) <= 3e-6
assert abs(e(V)[2,2] - +1.3ee1529cf1611X-005) <= 3e-6

_stfixest_estimate_ols_test y c.x, absorb(group) fweight(fw_valid)
assert e(N) == 26 & e(df_r) == 19 & e(sumweights) == 26
assert `"`e(wtype)'"' == "fweight" & `"`e(wexp)'"' == "= fw_valid"
assert abs(e(b)[1,1] - +1.071c74a8cc936X+001) <= 3e-6
assert abs(e(V)[1,1] - +1.37267e35e5f40X-007) <= 3e-6
assert abs(e(V)[2,2] - +1.85bac823e9064X-009) <= 3e-6

_stfixest_estimate_ols_test y c.x, absorb(group) pweight(pw_valid)
assert e(N) == 12 & e(df_r) == 5
assert abs(e(sumweights) - 12.4) <= 1e-6
assert `"`e(wtype)'"' == "pweight" & `"`e(wexp)'"' == "= pw_valid"
assert `"`e(vce)'"' == "robust" & `"`e(vcetype)'"' == "Robust"
assert abs(e(b)[1,1] - +1.0a633d2c1dd4cX+001) <= 3e-6
assert abs(e(V)[1,1] - +1.464d18a4af078X-005) <= 3e-6
assert abs(e(V)[2,2] - +1.6ca0cbb6a2a2aX-007) <= 3e-6

di as result "END_FIXTURE|estimate_ols_hidden_parity|rc=0"
