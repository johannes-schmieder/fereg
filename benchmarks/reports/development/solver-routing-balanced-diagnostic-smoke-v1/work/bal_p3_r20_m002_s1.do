version 18.0
clear all
set more off
set linesize 255
set rng mt64
set processors 8
adopath ++ "/Users/johannes/Git/fereg/stata"
adopath ++ "/Users/johannes/Git/fereg/dist"
set seed 26228582
set obs 24000
generate long obs_id = _n
generate long worker = ceil(_n / 3)
bysort worker: generate int period = _n
bysort worker (period): generate long firm = mod(7919*worker - 1, 4000) + 1 if _n == 1
bysort worker (period): replace firm = cond(runiform() < 0.02, mod(104729*worker + 7919*period - 1, 4000) + 1, firm[_n-1]) if _n > 1
sort worker firm obs_id
by worker firm: generate byte __edge_first = (_n == 1)
quietly count if __edge_first
scalar __graph_unique_edges = r(N)
scalar __graph_duplicate_fraction = 1 - __graph_unique_edges / _N
scalar __graph_edges_per_vertex = __graph_unique_edges / (8000 + 4000)
bysort worker: egen int __worker_degree = total(__edge_first)
by worker: generate byte __worker_first = (_n == 1)
quietly summarize __worker_degree if __worker_first, detail
scalar __graph_worker_degree_mean = r(mean)
scalar __graph_worker_degree_p50 = r(p50)
scalar __graph_worker_degree_p90 = r(p90)
scalar __graph_worker_degree_max = r(max)
bysort firm: egen int __firm_degree = total(__edge_first)
by firm: generate byte __firm_first = (_n == 1)
quietly summarize __firm_degree if __firm_first, detail
scalar __graph_firm_degree_mean = r(mean)
scalar __graph_firm_degree_p50 = r(p50)
scalar __graph_firm_degree_p90 = r(p90)
scalar __graph_firm_degree_max = r(max)
drop __edge_first __worker_degree __worker_first __firm_degree __firm_first
sort obs_id
bysort worker: generate double __worker_fe = rnormal(0,.4) if _n == 1
bysort worker: replace __worker_fe = __worker_fe[1]
bysort firm: generate double __firm_fe = rnormal(0,.2) if _n == 1
bysort firm: replace __firm_fe = __firm_fe[1]
generate double __xb = 0
generate double x1 = rnormal()
replace __xb = __xb + (0.29999999999999999)*x1
generate double x2 = rnormal()
replace __xb = __xb + (-0.10000000000000001)*x2
generate double x3 = rnormal()
replace __xb = __xb + (0.099999999999999992)*x3
generate double x4 = rnormal()
replace __xb = __xb + (-0.050000000000000003)*x4
generate double y = 1 + __xb + __worker_fe + __firm_fe + rnormal(0,.5)
sort obs_id
assert _N == 24000
postfile __runs str48 case_id str20 test_id str40 topology str64 topology_detail long seed long n int periods long workers long firms long third_levels byte fe_dimensions double mobility int controls int rhs int threads str32 absorb_order double unique_edges duplicate_edge_fraction edges_per_vertex worker_degree_mean worker_degree_p50 worker_degree_p90 worker_degree_max firm_degree_mean firm_degree_p50 firm_degree_p90 firm_degree_max int repetition int sequence str28 arm_id str12 requested_solver str16 requested_preconditioner double requested_tolerance int rc double command_seconds requested_solver_code used_solver_code iterations stop_reason certificate_passed absolute_infinity_norm relative_infinity_norm relative_two_norm normal_equation_residual solver_setup_seconds solver_solve_seconds fallback warning_count requested_preconditioner_code used_preconditioner_code pilot_iterations pilot_contraction residual_used_solver_code residual_iterations residual_fallback residual_pilot_iterations residual_pilot_contraction native_seconds precommit_seconds effective_threads reported_tolerance retained_level_1 retained_level_2 retained_level_3 rss b1 b2 b3 b4 graph_components str48 warning using "/Users/johannes/Git/fereg/benchmarks/reports/development/solver-routing-balanced-diagnostic-smoke-v1/work/bal_p3_r20_m002_s1_runs.dta", replace
capture quietly fereg y x1 x2 x3 x4, absorb(worker firm) dof(pairwise) solver(auto) threads(8) tolerance(1e-08) iterate(16000)
capture quietly fereg y x1 x2 x3 x4, absorb(worker firm) dof(pairwise) solver(map) threads(8) tolerance(1e-08) iterate(16000)
capture quietly fereg y x1 x2 x3 x4, absorb(worker firm) dof(pairwise) solver(cg) threads(8) tolerance(1e-08) iterate(16000)
capture quietly fereg y x1 x2 x3 x4, absorb(worker firm) dof(pairwise) solver(lsmr) threads(8) tolerance(1e-08) iterate(16000) preconditioner(schwarz)
capture quietly fereg y x1 x2 x3 x4, absorb(worker firm) dof(pairwise) solver(lsmr) threads(8) tolerance(1e-10) iterate(16000) preconditioner(schwarz)
capture quietly fereg y x1 x2 x3 x4, absorb(worker firm) dof(pairwise) solver(cmg) threads(8) tolerance(1e-08) iterate(16000) preconditioner(off)
timer clear 91
timer on 91
capture quietly fereg y x1 x2 x3 x4, absorb(worker firm) dof(pairwise) solver(auto) threads(8) tolerance(1e-08) iterate(16000)
scalar __rc = _rc
timer off 91
quietly timer list 91
scalar __seconds = r(t91)
timer clear 91
foreach __name in req used iterations stop certified absinf relinf rel2 normal setup solve fallback warnings preq pused pilot_iters pilot_contract res_used res_iters res_fallback res_pilot_iters res_pilot_contract native precommit effective reported_tol level1 level2 level3 rss b1 b2 b3 b4 graph_components {
    scalar __`__name' = .
}
if __rc == 0 {
    matrix __solver = e(fereg_solver)
    matrix __runtime = e(fereg_runtime)
    matrix __fe = e(fereg_fe)
    matrix __dof = e(dof_table)
    scalar __req = __solver[1,1]
    scalar __used = __solver[1,2]
    scalar __iterations = __solver[1,3]
    scalar __stop = __solver[1,4]
    scalar __certified = inlist(__stop,1,2)
    scalar __absinf = __solver[1,5]
    scalar __relinf = __solver[1,6]
    scalar __rel2 = __solver[1,7]
    scalar __normal = __solver[1,8]
    scalar __setup = __solver[1,9]
    scalar __solve = __solver[1,10]
    scalar __fallback = __solver[1,11]
    scalar __warnings = __solver[1,12]
    scalar __preq = __solver[1,13]
    scalar __pused = __solver[1,14]
    scalar __pilot_iters = __solver[1,15]
    scalar __pilot_contract = __solver[1,16]
    scalar __res_used = __solver[2,2]
    scalar __res_iters = __solver[2,3]
    scalar __res_fallback = __solver[2,11]
    scalar __res_pilot_iters = __solver[2,15]
    scalar __res_pilot_contract = __solver[2,16]
    scalar __native = __runtime[20,2]
    scalar __precommit = __runtime[21,2]
    scalar __effective = __runtime[29,2]
    scalar __reported_tol = __runtime[23,2]
    if rowsof(__fe) >= 1 scalar __level1 = __fe[1,4]
    if rowsof(__fe) >= 2 scalar __level2 = __fe[2,4]
    if rowsof(__fe) >= 3 scalar __level3 = __fe[3,4]
    if rowsof(__dof) >= 2 scalar __graph_components = __dof[2,2]
    scalar __rss = e(rss)
    capture scalar __b1 = _b[x1]
    capture scalar __b2 = _b[x2]
    capture scalar __b3 = _b[x3]
    capture scalar __b4 = _b[x4]
}
local __warning = cond(__fallback == 1 & __req == 1 & __used == 4 & __pilot_contract >= .008, "auto_lsmr_failed_then_cg", cond(__fallback == 1 & __req == 7 & __used == 4, "cmg_failed_then_cg", cond(__warnings > 0, "solver_warning", "")))
post __runs ("bal_p3_r20_m002_s1") ("balanced_diagnostic") ("balanced_worker_firm") ("periods3_ratio2_mobility0.02_seed1") (26228582) (24000) (3) (8000) (4000) (0) (2) (0.02) (4) (5) (8) ("worker firm") (__graph_unique_edges) (__graph_duplicate_fraction) (__graph_edges_per_vertex) (__graph_worker_degree_mean) (__graph_worker_degree_p50) (__graph_worker_degree_p90) (__graph_worker_degree_max) (__graph_firm_degree_mean) (__graph_firm_degree_p50) (__graph_firm_degree_p90) (__graph_firm_degree_max) (1) (1) ("auto") ("auto") ("none") (1e-08) (__rc) (__seconds) (__req) (__used) (__iterations) (__stop) (__certified) (__absinf) (__relinf) (__rel2) (__normal) (__setup) (__solve) (__fallback) (__warnings) (__preq) (__pused) (__pilot_iters) (__pilot_contract) (__res_used) (__res_iters) (__res_fallback) (__res_pilot_iters) (__res_pilot_contract) (__native) (__precommit) (__effective) (__reported_tol) (__level1) (__level2) (__level3) (__rss) (__b1) (__b2) (__b3) (__b4) (__graph_components) (`"`__warning'"')
timer clear 91
timer on 91
capture quietly fereg y x1 x2 x3 x4, absorb(worker firm) dof(pairwise) solver(map) threads(8) tolerance(1e-08) iterate(16000)
scalar __rc = _rc
timer off 91
quietly timer list 91
scalar __seconds = r(t91)
timer clear 91
foreach __name in req used iterations stop certified absinf relinf rel2 normal setup solve fallback warnings preq pused pilot_iters pilot_contract res_used res_iters res_fallback res_pilot_iters res_pilot_contract native precommit effective reported_tol level1 level2 level3 rss b1 b2 b3 b4 graph_components {
    scalar __`__name' = .
}
if __rc == 0 {
    matrix __solver = e(fereg_solver)
    matrix __runtime = e(fereg_runtime)
    matrix __fe = e(fereg_fe)
    matrix __dof = e(dof_table)
    scalar __req = __solver[1,1]
    scalar __used = __solver[1,2]
    scalar __iterations = __solver[1,3]
    scalar __stop = __solver[1,4]
    scalar __certified = inlist(__stop,1,2)
    scalar __absinf = __solver[1,5]
    scalar __relinf = __solver[1,6]
    scalar __rel2 = __solver[1,7]
    scalar __normal = __solver[1,8]
    scalar __setup = __solver[1,9]
    scalar __solve = __solver[1,10]
    scalar __fallback = __solver[1,11]
    scalar __warnings = __solver[1,12]
    scalar __preq = __solver[1,13]
    scalar __pused = __solver[1,14]
    scalar __pilot_iters = __solver[1,15]
    scalar __pilot_contract = __solver[1,16]
    scalar __res_used = __solver[2,2]
    scalar __res_iters = __solver[2,3]
    scalar __res_fallback = __solver[2,11]
    scalar __res_pilot_iters = __solver[2,15]
    scalar __res_pilot_contract = __solver[2,16]
    scalar __native = __runtime[20,2]
    scalar __precommit = __runtime[21,2]
    scalar __effective = __runtime[29,2]
    scalar __reported_tol = __runtime[23,2]
    if rowsof(__fe) >= 1 scalar __level1 = __fe[1,4]
    if rowsof(__fe) >= 2 scalar __level2 = __fe[2,4]
    if rowsof(__fe) >= 3 scalar __level3 = __fe[3,4]
    if rowsof(__dof) >= 2 scalar __graph_components = __dof[2,2]
    scalar __rss = e(rss)
    capture scalar __b1 = _b[x1]
    capture scalar __b2 = _b[x2]
    capture scalar __b3 = _b[x3]
    capture scalar __b4 = _b[x4]
}
local __warning = cond(__fallback == 1 & __req == 1 & __used == 4 & __pilot_contract >= .008, "auto_lsmr_failed_then_cg", cond(__fallback == 1 & __req == 7 & __used == 4, "cmg_failed_then_cg", cond(__warnings > 0, "solver_warning", "")))
post __runs ("bal_p3_r20_m002_s1") ("balanced_diagnostic") ("balanced_worker_firm") ("periods3_ratio2_mobility0.02_seed1") (26228582) (24000) (3) (8000) (4000) (0) (2) (0.02) (4) (5) (8) ("worker firm") (__graph_unique_edges) (__graph_duplicate_fraction) (__graph_edges_per_vertex) (__graph_worker_degree_mean) (__graph_worker_degree_p50) (__graph_worker_degree_p90) (__graph_worker_degree_max) (__graph_firm_degree_mean) (__graph_firm_degree_p50) (__graph_firm_degree_p90) (__graph_firm_degree_max) (1) (2) ("map") ("map") ("none") (1e-08) (__rc) (__seconds) (__req) (__used) (__iterations) (__stop) (__certified) (__absinf) (__relinf) (__rel2) (__normal) (__setup) (__solve) (__fallback) (__warnings) (__preq) (__pused) (__pilot_iters) (__pilot_contract) (__res_used) (__res_iters) (__res_fallback) (__res_pilot_iters) (__res_pilot_contract) (__native) (__precommit) (__effective) (__reported_tol) (__level1) (__level2) (__level3) (__rss) (__b1) (__b2) (__b3) (__b4) (__graph_components) (`"`__warning'"')
timer clear 91
timer on 91
capture quietly fereg y x1 x2 x3 x4, absorb(worker firm) dof(pairwise) solver(cg) threads(8) tolerance(1e-08) iterate(16000)
scalar __rc = _rc
timer off 91
quietly timer list 91
scalar __seconds = r(t91)
timer clear 91
foreach __name in req used iterations stop certified absinf relinf rel2 normal setup solve fallback warnings preq pused pilot_iters pilot_contract res_used res_iters res_fallback res_pilot_iters res_pilot_contract native precommit effective reported_tol level1 level2 level3 rss b1 b2 b3 b4 graph_components {
    scalar __`__name' = .
}
if __rc == 0 {
    matrix __solver = e(fereg_solver)
    matrix __runtime = e(fereg_runtime)
    matrix __fe = e(fereg_fe)
    matrix __dof = e(dof_table)
    scalar __req = __solver[1,1]
    scalar __used = __solver[1,2]
    scalar __iterations = __solver[1,3]
    scalar __stop = __solver[1,4]
    scalar __certified = inlist(__stop,1,2)
    scalar __absinf = __solver[1,5]
    scalar __relinf = __solver[1,6]
    scalar __rel2 = __solver[1,7]
    scalar __normal = __solver[1,8]
    scalar __setup = __solver[1,9]
    scalar __solve = __solver[1,10]
    scalar __fallback = __solver[1,11]
    scalar __warnings = __solver[1,12]
    scalar __preq = __solver[1,13]
    scalar __pused = __solver[1,14]
    scalar __pilot_iters = __solver[1,15]
    scalar __pilot_contract = __solver[1,16]
    scalar __res_used = __solver[2,2]
    scalar __res_iters = __solver[2,3]
    scalar __res_fallback = __solver[2,11]
    scalar __res_pilot_iters = __solver[2,15]
    scalar __res_pilot_contract = __solver[2,16]
    scalar __native = __runtime[20,2]
    scalar __precommit = __runtime[21,2]
    scalar __effective = __runtime[29,2]
    scalar __reported_tol = __runtime[23,2]
    if rowsof(__fe) >= 1 scalar __level1 = __fe[1,4]
    if rowsof(__fe) >= 2 scalar __level2 = __fe[2,4]
    if rowsof(__fe) >= 3 scalar __level3 = __fe[3,4]
    if rowsof(__dof) >= 2 scalar __graph_components = __dof[2,2]
    scalar __rss = e(rss)
    capture scalar __b1 = _b[x1]
    capture scalar __b2 = _b[x2]
    capture scalar __b3 = _b[x3]
    capture scalar __b4 = _b[x4]
}
local __warning = cond(__fallback == 1 & __req == 1 & __used == 4 & __pilot_contract >= .008, "auto_lsmr_failed_then_cg", cond(__fallback == 1 & __req == 7 & __used == 4, "cmg_failed_then_cg", cond(__warnings > 0, "solver_warning", "")))
post __runs ("bal_p3_r20_m002_s1") ("balanced_diagnostic") ("balanced_worker_firm") ("periods3_ratio2_mobility0.02_seed1") (26228582) (24000) (3) (8000) (4000) (0) (2) (0.02) (4) (5) (8) ("worker firm") (__graph_unique_edges) (__graph_duplicate_fraction) (__graph_edges_per_vertex) (__graph_worker_degree_mean) (__graph_worker_degree_p50) (__graph_worker_degree_p90) (__graph_worker_degree_max) (__graph_firm_degree_mean) (__graph_firm_degree_p50) (__graph_firm_degree_p90) (__graph_firm_degree_max) (1) (3) ("cg") ("cg") ("none") (1e-08) (__rc) (__seconds) (__req) (__used) (__iterations) (__stop) (__certified) (__absinf) (__relinf) (__rel2) (__normal) (__setup) (__solve) (__fallback) (__warnings) (__preq) (__pused) (__pilot_iters) (__pilot_contract) (__res_used) (__res_iters) (__res_fallback) (__res_pilot_iters) (__res_pilot_contract) (__native) (__precommit) (__effective) (__reported_tol) (__level1) (__level2) (__level3) (__rss) (__b1) (__b2) (__b3) (__b4) (__graph_components) (`"`__warning'"')
timer clear 91
timer on 91
capture quietly fereg y x1 x2 x3 x4, absorb(worker firm) dof(pairwise) solver(lsmr) threads(8) tolerance(1e-08) iterate(16000) preconditioner(schwarz)
scalar __rc = _rc
timer off 91
quietly timer list 91
scalar __seconds = r(t91)
timer clear 91
foreach __name in req used iterations stop certified absinf relinf rel2 normal setup solve fallback warnings preq pused pilot_iters pilot_contract res_used res_iters res_fallback res_pilot_iters res_pilot_contract native precommit effective reported_tol level1 level2 level3 rss b1 b2 b3 b4 graph_components {
    scalar __`__name' = .
}
if __rc == 0 {
    matrix __solver = e(fereg_solver)
    matrix __runtime = e(fereg_runtime)
    matrix __fe = e(fereg_fe)
    matrix __dof = e(dof_table)
    scalar __req = __solver[1,1]
    scalar __used = __solver[1,2]
    scalar __iterations = __solver[1,3]
    scalar __stop = __solver[1,4]
    scalar __certified = inlist(__stop,1,2)
    scalar __absinf = __solver[1,5]
    scalar __relinf = __solver[1,6]
    scalar __rel2 = __solver[1,7]
    scalar __normal = __solver[1,8]
    scalar __setup = __solver[1,9]
    scalar __solve = __solver[1,10]
    scalar __fallback = __solver[1,11]
    scalar __warnings = __solver[1,12]
    scalar __preq = __solver[1,13]
    scalar __pused = __solver[1,14]
    scalar __pilot_iters = __solver[1,15]
    scalar __pilot_contract = __solver[1,16]
    scalar __res_used = __solver[2,2]
    scalar __res_iters = __solver[2,3]
    scalar __res_fallback = __solver[2,11]
    scalar __res_pilot_iters = __solver[2,15]
    scalar __res_pilot_contract = __solver[2,16]
    scalar __native = __runtime[20,2]
    scalar __precommit = __runtime[21,2]
    scalar __effective = __runtime[29,2]
    scalar __reported_tol = __runtime[23,2]
    if rowsof(__fe) >= 1 scalar __level1 = __fe[1,4]
    if rowsof(__fe) >= 2 scalar __level2 = __fe[2,4]
    if rowsof(__fe) >= 3 scalar __level3 = __fe[3,4]
    if rowsof(__dof) >= 2 scalar __graph_components = __dof[2,2]
    scalar __rss = e(rss)
    capture scalar __b1 = _b[x1]
    capture scalar __b2 = _b[x2]
    capture scalar __b3 = _b[x3]
    capture scalar __b4 = _b[x4]
}
local __warning = cond(__fallback == 1 & __req == 1 & __used == 4 & __pilot_contract >= .008, "auto_lsmr_failed_then_cg", cond(__fallback == 1 & __req == 7 & __used == 4, "cmg_failed_then_cg", cond(__warnings > 0, "solver_warning", "")))
post __runs ("bal_p3_r20_m002_s1") ("balanced_diagnostic") ("balanced_worker_firm") ("periods3_ratio2_mobility0.02_seed1") (26228582) (24000) (3) (8000) (4000) (0) (2) (0.02) (4) (5) (8) ("worker firm") (__graph_unique_edges) (__graph_duplicate_fraction) (__graph_edges_per_vertex) (__graph_worker_degree_mean) (__graph_worker_degree_p50) (__graph_worker_degree_p90) (__graph_worker_degree_max) (__graph_firm_degree_mean) (__graph_firm_degree_p50) (__graph_firm_degree_p90) (__graph_firm_degree_max) (1) (4) ("lsmr_schwarz_1e8") ("lsmr") ("schwarz") (1e-08) (__rc) (__seconds) (__req) (__used) (__iterations) (__stop) (__certified) (__absinf) (__relinf) (__rel2) (__normal) (__setup) (__solve) (__fallback) (__warnings) (__preq) (__pused) (__pilot_iters) (__pilot_contract) (__res_used) (__res_iters) (__res_fallback) (__res_pilot_iters) (__res_pilot_contract) (__native) (__precommit) (__effective) (__reported_tol) (__level1) (__level2) (__level3) (__rss) (__b1) (__b2) (__b3) (__b4) (__graph_components) (`"`__warning'"')
timer clear 91
timer on 91
capture quietly fereg y x1 x2 x3 x4, absorb(worker firm) dof(pairwise) solver(lsmr) threads(8) tolerance(1e-10) iterate(16000) preconditioner(schwarz)
scalar __rc = _rc
timer off 91
quietly timer list 91
scalar __seconds = r(t91)
timer clear 91
foreach __name in req used iterations stop certified absinf relinf rel2 normal setup solve fallback warnings preq pused pilot_iters pilot_contract res_used res_iters res_fallback res_pilot_iters res_pilot_contract native precommit effective reported_tol level1 level2 level3 rss b1 b2 b3 b4 graph_components {
    scalar __`__name' = .
}
if __rc == 0 {
    matrix __solver = e(fereg_solver)
    matrix __runtime = e(fereg_runtime)
    matrix __fe = e(fereg_fe)
    matrix __dof = e(dof_table)
    scalar __req = __solver[1,1]
    scalar __used = __solver[1,2]
    scalar __iterations = __solver[1,3]
    scalar __stop = __solver[1,4]
    scalar __certified = inlist(__stop,1,2)
    scalar __absinf = __solver[1,5]
    scalar __relinf = __solver[1,6]
    scalar __rel2 = __solver[1,7]
    scalar __normal = __solver[1,8]
    scalar __setup = __solver[1,9]
    scalar __solve = __solver[1,10]
    scalar __fallback = __solver[1,11]
    scalar __warnings = __solver[1,12]
    scalar __preq = __solver[1,13]
    scalar __pused = __solver[1,14]
    scalar __pilot_iters = __solver[1,15]
    scalar __pilot_contract = __solver[1,16]
    scalar __res_used = __solver[2,2]
    scalar __res_iters = __solver[2,3]
    scalar __res_fallback = __solver[2,11]
    scalar __res_pilot_iters = __solver[2,15]
    scalar __res_pilot_contract = __solver[2,16]
    scalar __native = __runtime[20,2]
    scalar __precommit = __runtime[21,2]
    scalar __effective = __runtime[29,2]
    scalar __reported_tol = __runtime[23,2]
    if rowsof(__fe) >= 1 scalar __level1 = __fe[1,4]
    if rowsof(__fe) >= 2 scalar __level2 = __fe[2,4]
    if rowsof(__fe) >= 3 scalar __level3 = __fe[3,4]
    if rowsof(__dof) >= 2 scalar __graph_components = __dof[2,2]
    scalar __rss = e(rss)
    capture scalar __b1 = _b[x1]
    capture scalar __b2 = _b[x2]
    capture scalar __b3 = _b[x3]
    capture scalar __b4 = _b[x4]
}
local __warning = cond(__fallback == 1 & __req == 1 & __used == 4 & __pilot_contract >= .008, "auto_lsmr_failed_then_cg", cond(__fallback == 1 & __req == 7 & __used == 4, "cmg_failed_then_cg", cond(__warnings > 0, "solver_warning", "")))
post __runs ("bal_p3_r20_m002_s1") ("balanced_diagnostic") ("balanced_worker_firm") ("periods3_ratio2_mobility0.02_seed1") (26228582) (24000) (3) (8000) (4000) (0) (2) (0.02) (4) (5) (8) ("worker firm") (__graph_unique_edges) (__graph_duplicate_fraction) (__graph_edges_per_vertex) (__graph_worker_degree_mean) (__graph_worker_degree_p50) (__graph_worker_degree_p90) (__graph_worker_degree_max) (__graph_firm_degree_mean) (__graph_firm_degree_p50) (__graph_firm_degree_p90) (__graph_firm_degree_max) (1) (5) ("lsmr_schwarz_1e10") ("lsmr") ("schwarz") (1e-10) (__rc) (__seconds) (__req) (__used) (__iterations) (__stop) (__certified) (__absinf) (__relinf) (__rel2) (__normal) (__setup) (__solve) (__fallback) (__warnings) (__preq) (__pused) (__pilot_iters) (__pilot_contract) (__res_used) (__res_iters) (__res_fallback) (__res_pilot_iters) (__res_pilot_contract) (__native) (__precommit) (__effective) (__reported_tol) (__level1) (__level2) (__level3) (__rss) (__b1) (__b2) (__b3) (__b4) (__graph_components) (`"`__warning'"')
timer clear 91
timer on 91
capture quietly fereg y x1 x2 x3 x4, absorb(worker firm) dof(pairwise) solver(cmg) threads(8) tolerance(1e-08) iterate(16000) preconditioner(off)
scalar __rc = _rc
timer off 91
quietly timer list 91
scalar __seconds = r(t91)
timer clear 91
foreach __name in req used iterations stop certified absinf relinf rel2 normal setup solve fallback warnings preq pused pilot_iters pilot_contract res_used res_iters res_fallback res_pilot_iters res_pilot_contract native precommit effective reported_tol level1 level2 level3 rss b1 b2 b3 b4 graph_components {
    scalar __`__name' = .
}
if __rc == 0 {
    matrix __solver = e(fereg_solver)
    matrix __runtime = e(fereg_runtime)
    matrix __fe = e(fereg_fe)
    matrix __dof = e(dof_table)
    scalar __req = __solver[1,1]
    scalar __used = __solver[1,2]
    scalar __iterations = __solver[1,3]
    scalar __stop = __solver[1,4]
    scalar __certified = inlist(__stop,1,2)
    scalar __absinf = __solver[1,5]
    scalar __relinf = __solver[1,6]
    scalar __rel2 = __solver[1,7]
    scalar __normal = __solver[1,8]
    scalar __setup = __solver[1,9]
    scalar __solve = __solver[1,10]
    scalar __fallback = __solver[1,11]
    scalar __warnings = __solver[1,12]
    scalar __preq = __solver[1,13]
    scalar __pused = __solver[1,14]
    scalar __pilot_iters = __solver[1,15]
    scalar __pilot_contract = __solver[1,16]
    scalar __res_used = __solver[2,2]
    scalar __res_iters = __solver[2,3]
    scalar __res_fallback = __solver[2,11]
    scalar __res_pilot_iters = __solver[2,15]
    scalar __res_pilot_contract = __solver[2,16]
    scalar __native = __runtime[20,2]
    scalar __precommit = __runtime[21,2]
    scalar __effective = __runtime[29,2]
    scalar __reported_tol = __runtime[23,2]
    if rowsof(__fe) >= 1 scalar __level1 = __fe[1,4]
    if rowsof(__fe) >= 2 scalar __level2 = __fe[2,4]
    if rowsof(__fe) >= 3 scalar __level3 = __fe[3,4]
    if rowsof(__dof) >= 2 scalar __graph_components = __dof[2,2]
    scalar __rss = e(rss)
    capture scalar __b1 = _b[x1]
    capture scalar __b2 = _b[x2]
    capture scalar __b3 = _b[x3]
    capture scalar __b4 = _b[x4]
}
local __warning = cond(__fallback == 1 & __req == 1 & __used == 4 & __pilot_contract >= .008, "auto_lsmr_failed_then_cg", cond(__fallback == 1 & __req == 7 & __used == 4, "cmg_failed_then_cg", cond(__warnings > 0, "solver_warning", "")))
post __runs ("bal_p3_r20_m002_s1") ("balanced_diagnostic") ("balanced_worker_firm") ("periods3_ratio2_mobility0.02_seed1") (26228582) (24000) (3) (8000) (4000) (0) (2) (0.02) (4) (5) (8) ("worker firm") (__graph_unique_edges) (__graph_duplicate_fraction) (__graph_edges_per_vertex) (__graph_worker_degree_mean) (__graph_worker_degree_p50) (__graph_worker_degree_p90) (__graph_worker_degree_max) (__graph_firm_degree_mean) (__graph_firm_degree_p50) (__graph_firm_degree_p90) (__graph_firm_degree_max) (1) (6) ("cmg") ("cmg") ("off") (1e-08) (__rc) (__seconds) (__req) (__used) (__iterations) (__stop) (__certified) (__absinf) (__relinf) (__rel2) (__normal) (__setup) (__solve) (__fallback) (__warnings) (__preq) (__pused) (__pilot_iters) (__pilot_contract) (__res_used) (__res_iters) (__res_fallback) (__res_pilot_iters) (__res_pilot_contract) (__native) (__precommit) (__effective) (__reported_tol) (__level1) (__level2) (__level3) (__rss) (__b1) (__b2) (__b3) (__b4) (__graph_components) (`"`__warning'"')
timer clear 91
timer on 91
capture quietly fereg y x1 x2 x3 x4, absorb(worker firm) dof(pairwise) solver(map) threads(8) tolerance(1e-08) iterate(16000)
scalar __rc = _rc
timer off 91
quietly timer list 91
scalar __seconds = r(t91)
timer clear 91
foreach __name in req used iterations stop certified absinf relinf rel2 normal setup solve fallback warnings preq pused pilot_iters pilot_contract res_used res_iters res_fallback res_pilot_iters res_pilot_contract native precommit effective reported_tol level1 level2 level3 rss b1 b2 b3 b4 graph_components {
    scalar __`__name' = .
}
if __rc == 0 {
    matrix __solver = e(fereg_solver)
    matrix __runtime = e(fereg_runtime)
    matrix __fe = e(fereg_fe)
    matrix __dof = e(dof_table)
    scalar __req = __solver[1,1]
    scalar __used = __solver[1,2]
    scalar __iterations = __solver[1,3]
    scalar __stop = __solver[1,4]
    scalar __certified = inlist(__stop,1,2)
    scalar __absinf = __solver[1,5]
    scalar __relinf = __solver[1,6]
    scalar __rel2 = __solver[1,7]
    scalar __normal = __solver[1,8]
    scalar __setup = __solver[1,9]
    scalar __solve = __solver[1,10]
    scalar __fallback = __solver[1,11]
    scalar __warnings = __solver[1,12]
    scalar __preq = __solver[1,13]
    scalar __pused = __solver[1,14]
    scalar __pilot_iters = __solver[1,15]
    scalar __pilot_contract = __solver[1,16]
    scalar __res_used = __solver[2,2]
    scalar __res_iters = __solver[2,3]
    scalar __res_fallback = __solver[2,11]
    scalar __res_pilot_iters = __solver[2,15]
    scalar __res_pilot_contract = __solver[2,16]
    scalar __native = __runtime[20,2]
    scalar __precommit = __runtime[21,2]
    scalar __effective = __runtime[29,2]
    scalar __reported_tol = __runtime[23,2]
    if rowsof(__fe) >= 1 scalar __level1 = __fe[1,4]
    if rowsof(__fe) >= 2 scalar __level2 = __fe[2,4]
    if rowsof(__fe) >= 3 scalar __level3 = __fe[3,4]
    if rowsof(__dof) >= 2 scalar __graph_components = __dof[2,2]
    scalar __rss = e(rss)
    capture scalar __b1 = _b[x1]
    capture scalar __b2 = _b[x2]
    capture scalar __b3 = _b[x3]
    capture scalar __b4 = _b[x4]
}
local __warning = cond(__fallback == 1 & __req == 1 & __used == 4 & __pilot_contract >= .008, "auto_lsmr_failed_then_cg", cond(__fallback == 1 & __req == 7 & __used == 4, "cmg_failed_then_cg", cond(__warnings > 0, "solver_warning", "")))
post __runs ("bal_p3_r20_m002_s1") ("balanced_diagnostic") ("balanced_worker_firm") ("periods3_ratio2_mobility0.02_seed1") (26228582) (24000) (3) (8000) (4000) (0) (2) (0.02) (4) (5) (8) ("worker firm") (__graph_unique_edges) (__graph_duplicate_fraction) (__graph_edges_per_vertex) (__graph_worker_degree_mean) (__graph_worker_degree_p50) (__graph_worker_degree_p90) (__graph_worker_degree_max) (__graph_firm_degree_mean) (__graph_firm_degree_p50) (__graph_firm_degree_p90) (__graph_firm_degree_max) (2) (1) ("map") ("map") ("none") (1e-08) (__rc) (__seconds) (__req) (__used) (__iterations) (__stop) (__certified) (__absinf) (__relinf) (__rel2) (__normal) (__setup) (__solve) (__fallback) (__warnings) (__preq) (__pused) (__pilot_iters) (__pilot_contract) (__res_used) (__res_iters) (__res_fallback) (__res_pilot_iters) (__res_pilot_contract) (__native) (__precommit) (__effective) (__reported_tol) (__level1) (__level2) (__level3) (__rss) (__b1) (__b2) (__b3) (__b4) (__graph_components) (`"`__warning'"')
timer clear 91
timer on 91
capture quietly fereg y x1 x2 x3 x4, absorb(worker firm) dof(pairwise) solver(cg) threads(8) tolerance(1e-08) iterate(16000)
scalar __rc = _rc
timer off 91
quietly timer list 91
scalar __seconds = r(t91)
timer clear 91
foreach __name in req used iterations stop certified absinf relinf rel2 normal setup solve fallback warnings preq pused pilot_iters pilot_contract res_used res_iters res_fallback res_pilot_iters res_pilot_contract native precommit effective reported_tol level1 level2 level3 rss b1 b2 b3 b4 graph_components {
    scalar __`__name' = .
}
if __rc == 0 {
    matrix __solver = e(fereg_solver)
    matrix __runtime = e(fereg_runtime)
    matrix __fe = e(fereg_fe)
    matrix __dof = e(dof_table)
    scalar __req = __solver[1,1]
    scalar __used = __solver[1,2]
    scalar __iterations = __solver[1,3]
    scalar __stop = __solver[1,4]
    scalar __certified = inlist(__stop,1,2)
    scalar __absinf = __solver[1,5]
    scalar __relinf = __solver[1,6]
    scalar __rel2 = __solver[1,7]
    scalar __normal = __solver[1,8]
    scalar __setup = __solver[1,9]
    scalar __solve = __solver[1,10]
    scalar __fallback = __solver[1,11]
    scalar __warnings = __solver[1,12]
    scalar __preq = __solver[1,13]
    scalar __pused = __solver[1,14]
    scalar __pilot_iters = __solver[1,15]
    scalar __pilot_contract = __solver[1,16]
    scalar __res_used = __solver[2,2]
    scalar __res_iters = __solver[2,3]
    scalar __res_fallback = __solver[2,11]
    scalar __res_pilot_iters = __solver[2,15]
    scalar __res_pilot_contract = __solver[2,16]
    scalar __native = __runtime[20,2]
    scalar __precommit = __runtime[21,2]
    scalar __effective = __runtime[29,2]
    scalar __reported_tol = __runtime[23,2]
    if rowsof(__fe) >= 1 scalar __level1 = __fe[1,4]
    if rowsof(__fe) >= 2 scalar __level2 = __fe[2,4]
    if rowsof(__fe) >= 3 scalar __level3 = __fe[3,4]
    if rowsof(__dof) >= 2 scalar __graph_components = __dof[2,2]
    scalar __rss = e(rss)
    capture scalar __b1 = _b[x1]
    capture scalar __b2 = _b[x2]
    capture scalar __b3 = _b[x3]
    capture scalar __b4 = _b[x4]
}
local __warning = cond(__fallback == 1 & __req == 1 & __used == 4 & __pilot_contract >= .008, "auto_lsmr_failed_then_cg", cond(__fallback == 1 & __req == 7 & __used == 4, "cmg_failed_then_cg", cond(__warnings > 0, "solver_warning", "")))
post __runs ("bal_p3_r20_m002_s1") ("balanced_diagnostic") ("balanced_worker_firm") ("periods3_ratio2_mobility0.02_seed1") (26228582) (24000) (3) (8000) (4000) (0) (2) (0.02) (4) (5) (8) ("worker firm") (__graph_unique_edges) (__graph_duplicate_fraction) (__graph_edges_per_vertex) (__graph_worker_degree_mean) (__graph_worker_degree_p50) (__graph_worker_degree_p90) (__graph_worker_degree_max) (__graph_firm_degree_mean) (__graph_firm_degree_p50) (__graph_firm_degree_p90) (__graph_firm_degree_max) (2) (2) ("cg") ("cg") ("none") (1e-08) (__rc) (__seconds) (__req) (__used) (__iterations) (__stop) (__certified) (__absinf) (__relinf) (__rel2) (__normal) (__setup) (__solve) (__fallback) (__warnings) (__preq) (__pused) (__pilot_iters) (__pilot_contract) (__res_used) (__res_iters) (__res_fallback) (__res_pilot_iters) (__res_pilot_contract) (__native) (__precommit) (__effective) (__reported_tol) (__level1) (__level2) (__level3) (__rss) (__b1) (__b2) (__b3) (__b4) (__graph_components) (`"`__warning'"')
timer clear 91
timer on 91
capture quietly fereg y x1 x2 x3 x4, absorb(worker firm) dof(pairwise) solver(lsmr) threads(8) tolerance(1e-08) iterate(16000) preconditioner(schwarz)
scalar __rc = _rc
timer off 91
quietly timer list 91
scalar __seconds = r(t91)
timer clear 91
foreach __name in req used iterations stop certified absinf relinf rel2 normal setup solve fallback warnings preq pused pilot_iters pilot_contract res_used res_iters res_fallback res_pilot_iters res_pilot_contract native precommit effective reported_tol level1 level2 level3 rss b1 b2 b3 b4 graph_components {
    scalar __`__name' = .
}
if __rc == 0 {
    matrix __solver = e(fereg_solver)
    matrix __runtime = e(fereg_runtime)
    matrix __fe = e(fereg_fe)
    matrix __dof = e(dof_table)
    scalar __req = __solver[1,1]
    scalar __used = __solver[1,2]
    scalar __iterations = __solver[1,3]
    scalar __stop = __solver[1,4]
    scalar __certified = inlist(__stop,1,2)
    scalar __absinf = __solver[1,5]
    scalar __relinf = __solver[1,6]
    scalar __rel2 = __solver[1,7]
    scalar __normal = __solver[1,8]
    scalar __setup = __solver[1,9]
    scalar __solve = __solver[1,10]
    scalar __fallback = __solver[1,11]
    scalar __warnings = __solver[1,12]
    scalar __preq = __solver[1,13]
    scalar __pused = __solver[1,14]
    scalar __pilot_iters = __solver[1,15]
    scalar __pilot_contract = __solver[1,16]
    scalar __res_used = __solver[2,2]
    scalar __res_iters = __solver[2,3]
    scalar __res_fallback = __solver[2,11]
    scalar __res_pilot_iters = __solver[2,15]
    scalar __res_pilot_contract = __solver[2,16]
    scalar __native = __runtime[20,2]
    scalar __precommit = __runtime[21,2]
    scalar __effective = __runtime[29,2]
    scalar __reported_tol = __runtime[23,2]
    if rowsof(__fe) >= 1 scalar __level1 = __fe[1,4]
    if rowsof(__fe) >= 2 scalar __level2 = __fe[2,4]
    if rowsof(__fe) >= 3 scalar __level3 = __fe[3,4]
    if rowsof(__dof) >= 2 scalar __graph_components = __dof[2,2]
    scalar __rss = e(rss)
    capture scalar __b1 = _b[x1]
    capture scalar __b2 = _b[x2]
    capture scalar __b3 = _b[x3]
    capture scalar __b4 = _b[x4]
}
local __warning = cond(__fallback == 1 & __req == 1 & __used == 4 & __pilot_contract >= .008, "auto_lsmr_failed_then_cg", cond(__fallback == 1 & __req == 7 & __used == 4, "cmg_failed_then_cg", cond(__warnings > 0, "solver_warning", "")))
post __runs ("bal_p3_r20_m002_s1") ("balanced_diagnostic") ("balanced_worker_firm") ("periods3_ratio2_mobility0.02_seed1") (26228582) (24000) (3) (8000) (4000) (0) (2) (0.02) (4) (5) (8) ("worker firm") (__graph_unique_edges) (__graph_duplicate_fraction) (__graph_edges_per_vertex) (__graph_worker_degree_mean) (__graph_worker_degree_p50) (__graph_worker_degree_p90) (__graph_worker_degree_max) (__graph_firm_degree_mean) (__graph_firm_degree_p50) (__graph_firm_degree_p90) (__graph_firm_degree_max) (2) (3) ("lsmr_schwarz_1e8") ("lsmr") ("schwarz") (1e-08) (__rc) (__seconds) (__req) (__used) (__iterations) (__stop) (__certified) (__absinf) (__relinf) (__rel2) (__normal) (__setup) (__solve) (__fallback) (__warnings) (__preq) (__pused) (__pilot_iters) (__pilot_contract) (__res_used) (__res_iters) (__res_fallback) (__res_pilot_iters) (__res_pilot_contract) (__native) (__precommit) (__effective) (__reported_tol) (__level1) (__level2) (__level3) (__rss) (__b1) (__b2) (__b3) (__b4) (__graph_components) (`"`__warning'"')
timer clear 91
timer on 91
capture quietly fereg y x1 x2 x3 x4, absorb(worker firm) dof(pairwise) solver(lsmr) threads(8) tolerance(1e-10) iterate(16000) preconditioner(schwarz)
scalar __rc = _rc
timer off 91
quietly timer list 91
scalar __seconds = r(t91)
timer clear 91
foreach __name in req used iterations stop certified absinf relinf rel2 normal setup solve fallback warnings preq pused pilot_iters pilot_contract res_used res_iters res_fallback res_pilot_iters res_pilot_contract native precommit effective reported_tol level1 level2 level3 rss b1 b2 b3 b4 graph_components {
    scalar __`__name' = .
}
if __rc == 0 {
    matrix __solver = e(fereg_solver)
    matrix __runtime = e(fereg_runtime)
    matrix __fe = e(fereg_fe)
    matrix __dof = e(dof_table)
    scalar __req = __solver[1,1]
    scalar __used = __solver[1,2]
    scalar __iterations = __solver[1,3]
    scalar __stop = __solver[1,4]
    scalar __certified = inlist(__stop,1,2)
    scalar __absinf = __solver[1,5]
    scalar __relinf = __solver[1,6]
    scalar __rel2 = __solver[1,7]
    scalar __normal = __solver[1,8]
    scalar __setup = __solver[1,9]
    scalar __solve = __solver[1,10]
    scalar __fallback = __solver[1,11]
    scalar __warnings = __solver[1,12]
    scalar __preq = __solver[1,13]
    scalar __pused = __solver[1,14]
    scalar __pilot_iters = __solver[1,15]
    scalar __pilot_contract = __solver[1,16]
    scalar __res_used = __solver[2,2]
    scalar __res_iters = __solver[2,3]
    scalar __res_fallback = __solver[2,11]
    scalar __res_pilot_iters = __solver[2,15]
    scalar __res_pilot_contract = __solver[2,16]
    scalar __native = __runtime[20,2]
    scalar __precommit = __runtime[21,2]
    scalar __effective = __runtime[29,2]
    scalar __reported_tol = __runtime[23,2]
    if rowsof(__fe) >= 1 scalar __level1 = __fe[1,4]
    if rowsof(__fe) >= 2 scalar __level2 = __fe[2,4]
    if rowsof(__fe) >= 3 scalar __level3 = __fe[3,4]
    if rowsof(__dof) >= 2 scalar __graph_components = __dof[2,2]
    scalar __rss = e(rss)
    capture scalar __b1 = _b[x1]
    capture scalar __b2 = _b[x2]
    capture scalar __b3 = _b[x3]
    capture scalar __b4 = _b[x4]
}
local __warning = cond(__fallback == 1 & __req == 1 & __used == 4 & __pilot_contract >= .008, "auto_lsmr_failed_then_cg", cond(__fallback == 1 & __req == 7 & __used == 4, "cmg_failed_then_cg", cond(__warnings > 0, "solver_warning", "")))
post __runs ("bal_p3_r20_m002_s1") ("balanced_diagnostic") ("balanced_worker_firm") ("periods3_ratio2_mobility0.02_seed1") (26228582) (24000) (3) (8000) (4000) (0) (2) (0.02) (4) (5) (8) ("worker firm") (__graph_unique_edges) (__graph_duplicate_fraction) (__graph_edges_per_vertex) (__graph_worker_degree_mean) (__graph_worker_degree_p50) (__graph_worker_degree_p90) (__graph_worker_degree_max) (__graph_firm_degree_mean) (__graph_firm_degree_p50) (__graph_firm_degree_p90) (__graph_firm_degree_max) (2) (4) ("lsmr_schwarz_1e10") ("lsmr") ("schwarz") (1e-10) (__rc) (__seconds) (__req) (__used) (__iterations) (__stop) (__certified) (__absinf) (__relinf) (__rel2) (__normal) (__setup) (__solve) (__fallback) (__warnings) (__preq) (__pused) (__pilot_iters) (__pilot_contract) (__res_used) (__res_iters) (__res_fallback) (__res_pilot_iters) (__res_pilot_contract) (__native) (__precommit) (__effective) (__reported_tol) (__level1) (__level2) (__level3) (__rss) (__b1) (__b2) (__b3) (__b4) (__graph_components) (`"`__warning'"')
timer clear 91
timer on 91
capture quietly fereg y x1 x2 x3 x4, absorb(worker firm) dof(pairwise) solver(cmg) threads(8) tolerance(1e-08) iterate(16000) preconditioner(off)
scalar __rc = _rc
timer off 91
quietly timer list 91
scalar __seconds = r(t91)
timer clear 91
foreach __name in req used iterations stop certified absinf relinf rel2 normal setup solve fallback warnings preq pused pilot_iters pilot_contract res_used res_iters res_fallback res_pilot_iters res_pilot_contract native precommit effective reported_tol level1 level2 level3 rss b1 b2 b3 b4 graph_components {
    scalar __`__name' = .
}
if __rc == 0 {
    matrix __solver = e(fereg_solver)
    matrix __runtime = e(fereg_runtime)
    matrix __fe = e(fereg_fe)
    matrix __dof = e(dof_table)
    scalar __req = __solver[1,1]
    scalar __used = __solver[1,2]
    scalar __iterations = __solver[1,3]
    scalar __stop = __solver[1,4]
    scalar __certified = inlist(__stop,1,2)
    scalar __absinf = __solver[1,5]
    scalar __relinf = __solver[1,6]
    scalar __rel2 = __solver[1,7]
    scalar __normal = __solver[1,8]
    scalar __setup = __solver[1,9]
    scalar __solve = __solver[1,10]
    scalar __fallback = __solver[1,11]
    scalar __warnings = __solver[1,12]
    scalar __preq = __solver[1,13]
    scalar __pused = __solver[1,14]
    scalar __pilot_iters = __solver[1,15]
    scalar __pilot_contract = __solver[1,16]
    scalar __res_used = __solver[2,2]
    scalar __res_iters = __solver[2,3]
    scalar __res_fallback = __solver[2,11]
    scalar __res_pilot_iters = __solver[2,15]
    scalar __res_pilot_contract = __solver[2,16]
    scalar __native = __runtime[20,2]
    scalar __precommit = __runtime[21,2]
    scalar __effective = __runtime[29,2]
    scalar __reported_tol = __runtime[23,2]
    if rowsof(__fe) >= 1 scalar __level1 = __fe[1,4]
    if rowsof(__fe) >= 2 scalar __level2 = __fe[2,4]
    if rowsof(__fe) >= 3 scalar __level3 = __fe[3,4]
    if rowsof(__dof) >= 2 scalar __graph_components = __dof[2,2]
    scalar __rss = e(rss)
    capture scalar __b1 = _b[x1]
    capture scalar __b2 = _b[x2]
    capture scalar __b3 = _b[x3]
    capture scalar __b4 = _b[x4]
}
local __warning = cond(__fallback == 1 & __req == 1 & __used == 4 & __pilot_contract >= .008, "auto_lsmr_failed_then_cg", cond(__fallback == 1 & __req == 7 & __used == 4, "cmg_failed_then_cg", cond(__warnings > 0, "solver_warning", "")))
post __runs ("bal_p3_r20_m002_s1") ("balanced_diagnostic") ("balanced_worker_firm") ("periods3_ratio2_mobility0.02_seed1") (26228582) (24000) (3) (8000) (4000) (0) (2) (0.02) (4) (5) (8) ("worker firm") (__graph_unique_edges) (__graph_duplicate_fraction) (__graph_edges_per_vertex) (__graph_worker_degree_mean) (__graph_worker_degree_p50) (__graph_worker_degree_p90) (__graph_worker_degree_max) (__graph_firm_degree_mean) (__graph_firm_degree_p50) (__graph_firm_degree_p90) (__graph_firm_degree_max) (2) (5) ("cmg") ("cmg") ("off") (1e-08) (__rc) (__seconds) (__req) (__used) (__iterations) (__stop) (__certified) (__absinf) (__relinf) (__rel2) (__normal) (__setup) (__solve) (__fallback) (__warnings) (__preq) (__pused) (__pilot_iters) (__pilot_contract) (__res_used) (__res_iters) (__res_fallback) (__res_pilot_iters) (__res_pilot_contract) (__native) (__precommit) (__effective) (__reported_tol) (__level1) (__level2) (__level3) (__rss) (__b1) (__b2) (__b3) (__b4) (__graph_components) (`"`__warning'"')
timer clear 91
timer on 91
capture quietly fereg y x1 x2 x3 x4, absorb(worker firm) dof(pairwise) solver(auto) threads(8) tolerance(1e-08) iterate(16000)
scalar __rc = _rc
timer off 91
quietly timer list 91
scalar __seconds = r(t91)
timer clear 91
foreach __name in req used iterations stop certified absinf relinf rel2 normal setup solve fallback warnings preq pused pilot_iters pilot_contract res_used res_iters res_fallback res_pilot_iters res_pilot_contract native precommit effective reported_tol level1 level2 level3 rss b1 b2 b3 b4 graph_components {
    scalar __`__name' = .
}
if __rc == 0 {
    matrix __solver = e(fereg_solver)
    matrix __runtime = e(fereg_runtime)
    matrix __fe = e(fereg_fe)
    matrix __dof = e(dof_table)
    scalar __req = __solver[1,1]
    scalar __used = __solver[1,2]
    scalar __iterations = __solver[1,3]
    scalar __stop = __solver[1,4]
    scalar __certified = inlist(__stop,1,2)
    scalar __absinf = __solver[1,5]
    scalar __relinf = __solver[1,6]
    scalar __rel2 = __solver[1,7]
    scalar __normal = __solver[1,8]
    scalar __setup = __solver[1,9]
    scalar __solve = __solver[1,10]
    scalar __fallback = __solver[1,11]
    scalar __warnings = __solver[1,12]
    scalar __preq = __solver[1,13]
    scalar __pused = __solver[1,14]
    scalar __pilot_iters = __solver[1,15]
    scalar __pilot_contract = __solver[1,16]
    scalar __res_used = __solver[2,2]
    scalar __res_iters = __solver[2,3]
    scalar __res_fallback = __solver[2,11]
    scalar __res_pilot_iters = __solver[2,15]
    scalar __res_pilot_contract = __solver[2,16]
    scalar __native = __runtime[20,2]
    scalar __precommit = __runtime[21,2]
    scalar __effective = __runtime[29,2]
    scalar __reported_tol = __runtime[23,2]
    if rowsof(__fe) >= 1 scalar __level1 = __fe[1,4]
    if rowsof(__fe) >= 2 scalar __level2 = __fe[2,4]
    if rowsof(__fe) >= 3 scalar __level3 = __fe[3,4]
    if rowsof(__dof) >= 2 scalar __graph_components = __dof[2,2]
    scalar __rss = e(rss)
    capture scalar __b1 = _b[x1]
    capture scalar __b2 = _b[x2]
    capture scalar __b3 = _b[x3]
    capture scalar __b4 = _b[x4]
}
local __warning = cond(__fallback == 1 & __req == 1 & __used == 4 & __pilot_contract >= .008, "auto_lsmr_failed_then_cg", cond(__fallback == 1 & __req == 7 & __used == 4, "cmg_failed_then_cg", cond(__warnings > 0, "solver_warning", "")))
post __runs ("bal_p3_r20_m002_s1") ("balanced_diagnostic") ("balanced_worker_firm") ("periods3_ratio2_mobility0.02_seed1") (26228582) (24000) (3) (8000) (4000) (0) (2) (0.02) (4) (5) (8) ("worker firm") (__graph_unique_edges) (__graph_duplicate_fraction) (__graph_edges_per_vertex) (__graph_worker_degree_mean) (__graph_worker_degree_p50) (__graph_worker_degree_p90) (__graph_worker_degree_max) (__graph_firm_degree_mean) (__graph_firm_degree_p50) (__graph_firm_degree_p90) (__graph_firm_degree_max) (2) (6) ("auto") ("auto") ("none") (1e-08) (__rc) (__seconds) (__req) (__used) (__iterations) (__stop) (__certified) (__absinf) (__relinf) (__rel2) (__normal) (__setup) (__solve) (__fallback) (__warnings) (__preq) (__pused) (__pilot_iters) (__pilot_contract) (__res_used) (__res_iters) (__res_fallback) (__res_pilot_iters) (__res_pilot_contract) (__native) (__precommit) (__effective) (__reported_tol) (__level1) (__level2) (__level3) (__rss) (__b1) (__b2) (__b3) (__b4) (__graph_components) (`"`__warning'"')
timer clear 91
timer on 91
capture quietly fereg y x1 x2 x3 x4, absorb(worker firm) dof(pairwise) solver(cg) threads(8) tolerance(1e-08) iterate(16000)
scalar __rc = _rc
timer off 91
quietly timer list 91
scalar __seconds = r(t91)
timer clear 91
foreach __name in req used iterations stop certified absinf relinf rel2 normal setup solve fallback warnings preq pused pilot_iters pilot_contract res_used res_iters res_fallback res_pilot_iters res_pilot_contract native precommit effective reported_tol level1 level2 level3 rss b1 b2 b3 b4 graph_components {
    scalar __`__name' = .
}
if __rc == 0 {
    matrix __solver = e(fereg_solver)
    matrix __runtime = e(fereg_runtime)
    matrix __fe = e(fereg_fe)
    matrix __dof = e(dof_table)
    scalar __req = __solver[1,1]
    scalar __used = __solver[1,2]
    scalar __iterations = __solver[1,3]
    scalar __stop = __solver[1,4]
    scalar __certified = inlist(__stop,1,2)
    scalar __absinf = __solver[1,5]
    scalar __relinf = __solver[1,6]
    scalar __rel2 = __solver[1,7]
    scalar __normal = __solver[1,8]
    scalar __setup = __solver[1,9]
    scalar __solve = __solver[1,10]
    scalar __fallback = __solver[1,11]
    scalar __warnings = __solver[1,12]
    scalar __preq = __solver[1,13]
    scalar __pused = __solver[1,14]
    scalar __pilot_iters = __solver[1,15]
    scalar __pilot_contract = __solver[1,16]
    scalar __res_used = __solver[2,2]
    scalar __res_iters = __solver[2,3]
    scalar __res_fallback = __solver[2,11]
    scalar __res_pilot_iters = __solver[2,15]
    scalar __res_pilot_contract = __solver[2,16]
    scalar __native = __runtime[20,2]
    scalar __precommit = __runtime[21,2]
    scalar __effective = __runtime[29,2]
    scalar __reported_tol = __runtime[23,2]
    if rowsof(__fe) >= 1 scalar __level1 = __fe[1,4]
    if rowsof(__fe) >= 2 scalar __level2 = __fe[2,4]
    if rowsof(__fe) >= 3 scalar __level3 = __fe[3,4]
    if rowsof(__dof) >= 2 scalar __graph_components = __dof[2,2]
    scalar __rss = e(rss)
    capture scalar __b1 = _b[x1]
    capture scalar __b2 = _b[x2]
    capture scalar __b3 = _b[x3]
    capture scalar __b4 = _b[x4]
}
local __warning = cond(__fallback == 1 & __req == 1 & __used == 4 & __pilot_contract >= .008, "auto_lsmr_failed_then_cg", cond(__fallback == 1 & __req == 7 & __used == 4, "cmg_failed_then_cg", cond(__warnings > 0, "solver_warning", "")))
post __runs ("bal_p3_r20_m002_s1") ("balanced_diagnostic") ("balanced_worker_firm") ("periods3_ratio2_mobility0.02_seed1") (26228582) (24000) (3) (8000) (4000) (0) (2) (0.02) (4) (5) (8) ("worker firm") (__graph_unique_edges) (__graph_duplicate_fraction) (__graph_edges_per_vertex) (__graph_worker_degree_mean) (__graph_worker_degree_p50) (__graph_worker_degree_p90) (__graph_worker_degree_max) (__graph_firm_degree_mean) (__graph_firm_degree_p50) (__graph_firm_degree_p90) (__graph_firm_degree_max) (3) (1) ("cg") ("cg") ("none") (1e-08) (__rc) (__seconds) (__req) (__used) (__iterations) (__stop) (__certified) (__absinf) (__relinf) (__rel2) (__normal) (__setup) (__solve) (__fallback) (__warnings) (__preq) (__pused) (__pilot_iters) (__pilot_contract) (__res_used) (__res_iters) (__res_fallback) (__res_pilot_iters) (__res_pilot_contract) (__native) (__precommit) (__effective) (__reported_tol) (__level1) (__level2) (__level3) (__rss) (__b1) (__b2) (__b3) (__b4) (__graph_components) (`"`__warning'"')
timer clear 91
timer on 91
capture quietly fereg y x1 x2 x3 x4, absorb(worker firm) dof(pairwise) solver(lsmr) threads(8) tolerance(1e-08) iterate(16000) preconditioner(schwarz)
scalar __rc = _rc
timer off 91
quietly timer list 91
scalar __seconds = r(t91)
timer clear 91
foreach __name in req used iterations stop certified absinf relinf rel2 normal setup solve fallback warnings preq pused pilot_iters pilot_contract res_used res_iters res_fallback res_pilot_iters res_pilot_contract native precommit effective reported_tol level1 level2 level3 rss b1 b2 b3 b4 graph_components {
    scalar __`__name' = .
}
if __rc == 0 {
    matrix __solver = e(fereg_solver)
    matrix __runtime = e(fereg_runtime)
    matrix __fe = e(fereg_fe)
    matrix __dof = e(dof_table)
    scalar __req = __solver[1,1]
    scalar __used = __solver[1,2]
    scalar __iterations = __solver[1,3]
    scalar __stop = __solver[1,4]
    scalar __certified = inlist(__stop,1,2)
    scalar __absinf = __solver[1,5]
    scalar __relinf = __solver[1,6]
    scalar __rel2 = __solver[1,7]
    scalar __normal = __solver[1,8]
    scalar __setup = __solver[1,9]
    scalar __solve = __solver[1,10]
    scalar __fallback = __solver[1,11]
    scalar __warnings = __solver[1,12]
    scalar __preq = __solver[1,13]
    scalar __pused = __solver[1,14]
    scalar __pilot_iters = __solver[1,15]
    scalar __pilot_contract = __solver[1,16]
    scalar __res_used = __solver[2,2]
    scalar __res_iters = __solver[2,3]
    scalar __res_fallback = __solver[2,11]
    scalar __res_pilot_iters = __solver[2,15]
    scalar __res_pilot_contract = __solver[2,16]
    scalar __native = __runtime[20,2]
    scalar __precommit = __runtime[21,2]
    scalar __effective = __runtime[29,2]
    scalar __reported_tol = __runtime[23,2]
    if rowsof(__fe) >= 1 scalar __level1 = __fe[1,4]
    if rowsof(__fe) >= 2 scalar __level2 = __fe[2,4]
    if rowsof(__fe) >= 3 scalar __level3 = __fe[3,4]
    if rowsof(__dof) >= 2 scalar __graph_components = __dof[2,2]
    scalar __rss = e(rss)
    capture scalar __b1 = _b[x1]
    capture scalar __b2 = _b[x2]
    capture scalar __b3 = _b[x3]
    capture scalar __b4 = _b[x4]
}
local __warning = cond(__fallback == 1 & __req == 1 & __used == 4 & __pilot_contract >= .008, "auto_lsmr_failed_then_cg", cond(__fallback == 1 & __req == 7 & __used == 4, "cmg_failed_then_cg", cond(__warnings > 0, "solver_warning", "")))
post __runs ("bal_p3_r20_m002_s1") ("balanced_diagnostic") ("balanced_worker_firm") ("periods3_ratio2_mobility0.02_seed1") (26228582) (24000) (3) (8000) (4000) (0) (2) (0.02) (4) (5) (8) ("worker firm") (__graph_unique_edges) (__graph_duplicate_fraction) (__graph_edges_per_vertex) (__graph_worker_degree_mean) (__graph_worker_degree_p50) (__graph_worker_degree_p90) (__graph_worker_degree_max) (__graph_firm_degree_mean) (__graph_firm_degree_p50) (__graph_firm_degree_p90) (__graph_firm_degree_max) (3) (2) ("lsmr_schwarz_1e8") ("lsmr") ("schwarz") (1e-08) (__rc) (__seconds) (__req) (__used) (__iterations) (__stop) (__certified) (__absinf) (__relinf) (__rel2) (__normal) (__setup) (__solve) (__fallback) (__warnings) (__preq) (__pused) (__pilot_iters) (__pilot_contract) (__res_used) (__res_iters) (__res_fallback) (__res_pilot_iters) (__res_pilot_contract) (__native) (__precommit) (__effective) (__reported_tol) (__level1) (__level2) (__level3) (__rss) (__b1) (__b2) (__b3) (__b4) (__graph_components) (`"`__warning'"')
timer clear 91
timer on 91
capture quietly fereg y x1 x2 x3 x4, absorb(worker firm) dof(pairwise) solver(lsmr) threads(8) tolerance(1e-10) iterate(16000) preconditioner(schwarz)
scalar __rc = _rc
timer off 91
quietly timer list 91
scalar __seconds = r(t91)
timer clear 91
foreach __name in req used iterations stop certified absinf relinf rel2 normal setup solve fallback warnings preq pused pilot_iters pilot_contract res_used res_iters res_fallback res_pilot_iters res_pilot_contract native precommit effective reported_tol level1 level2 level3 rss b1 b2 b3 b4 graph_components {
    scalar __`__name' = .
}
if __rc == 0 {
    matrix __solver = e(fereg_solver)
    matrix __runtime = e(fereg_runtime)
    matrix __fe = e(fereg_fe)
    matrix __dof = e(dof_table)
    scalar __req = __solver[1,1]
    scalar __used = __solver[1,2]
    scalar __iterations = __solver[1,3]
    scalar __stop = __solver[1,4]
    scalar __certified = inlist(__stop,1,2)
    scalar __absinf = __solver[1,5]
    scalar __relinf = __solver[1,6]
    scalar __rel2 = __solver[1,7]
    scalar __normal = __solver[1,8]
    scalar __setup = __solver[1,9]
    scalar __solve = __solver[1,10]
    scalar __fallback = __solver[1,11]
    scalar __warnings = __solver[1,12]
    scalar __preq = __solver[1,13]
    scalar __pused = __solver[1,14]
    scalar __pilot_iters = __solver[1,15]
    scalar __pilot_contract = __solver[1,16]
    scalar __res_used = __solver[2,2]
    scalar __res_iters = __solver[2,3]
    scalar __res_fallback = __solver[2,11]
    scalar __res_pilot_iters = __solver[2,15]
    scalar __res_pilot_contract = __solver[2,16]
    scalar __native = __runtime[20,2]
    scalar __precommit = __runtime[21,2]
    scalar __effective = __runtime[29,2]
    scalar __reported_tol = __runtime[23,2]
    if rowsof(__fe) >= 1 scalar __level1 = __fe[1,4]
    if rowsof(__fe) >= 2 scalar __level2 = __fe[2,4]
    if rowsof(__fe) >= 3 scalar __level3 = __fe[3,4]
    if rowsof(__dof) >= 2 scalar __graph_components = __dof[2,2]
    scalar __rss = e(rss)
    capture scalar __b1 = _b[x1]
    capture scalar __b2 = _b[x2]
    capture scalar __b3 = _b[x3]
    capture scalar __b4 = _b[x4]
}
local __warning = cond(__fallback == 1 & __req == 1 & __used == 4 & __pilot_contract >= .008, "auto_lsmr_failed_then_cg", cond(__fallback == 1 & __req == 7 & __used == 4, "cmg_failed_then_cg", cond(__warnings > 0, "solver_warning", "")))
post __runs ("bal_p3_r20_m002_s1") ("balanced_diagnostic") ("balanced_worker_firm") ("periods3_ratio2_mobility0.02_seed1") (26228582) (24000) (3) (8000) (4000) (0) (2) (0.02) (4) (5) (8) ("worker firm") (__graph_unique_edges) (__graph_duplicate_fraction) (__graph_edges_per_vertex) (__graph_worker_degree_mean) (__graph_worker_degree_p50) (__graph_worker_degree_p90) (__graph_worker_degree_max) (__graph_firm_degree_mean) (__graph_firm_degree_p50) (__graph_firm_degree_p90) (__graph_firm_degree_max) (3) (3) ("lsmr_schwarz_1e10") ("lsmr") ("schwarz") (1e-10) (__rc) (__seconds) (__req) (__used) (__iterations) (__stop) (__certified) (__absinf) (__relinf) (__rel2) (__normal) (__setup) (__solve) (__fallback) (__warnings) (__preq) (__pused) (__pilot_iters) (__pilot_contract) (__res_used) (__res_iters) (__res_fallback) (__res_pilot_iters) (__res_pilot_contract) (__native) (__precommit) (__effective) (__reported_tol) (__level1) (__level2) (__level3) (__rss) (__b1) (__b2) (__b3) (__b4) (__graph_components) (`"`__warning'"')
timer clear 91
timer on 91
capture quietly fereg y x1 x2 x3 x4, absorb(worker firm) dof(pairwise) solver(cmg) threads(8) tolerance(1e-08) iterate(16000) preconditioner(off)
scalar __rc = _rc
timer off 91
quietly timer list 91
scalar __seconds = r(t91)
timer clear 91
foreach __name in req used iterations stop certified absinf relinf rel2 normal setup solve fallback warnings preq pused pilot_iters pilot_contract res_used res_iters res_fallback res_pilot_iters res_pilot_contract native precommit effective reported_tol level1 level2 level3 rss b1 b2 b3 b4 graph_components {
    scalar __`__name' = .
}
if __rc == 0 {
    matrix __solver = e(fereg_solver)
    matrix __runtime = e(fereg_runtime)
    matrix __fe = e(fereg_fe)
    matrix __dof = e(dof_table)
    scalar __req = __solver[1,1]
    scalar __used = __solver[1,2]
    scalar __iterations = __solver[1,3]
    scalar __stop = __solver[1,4]
    scalar __certified = inlist(__stop,1,2)
    scalar __absinf = __solver[1,5]
    scalar __relinf = __solver[1,6]
    scalar __rel2 = __solver[1,7]
    scalar __normal = __solver[1,8]
    scalar __setup = __solver[1,9]
    scalar __solve = __solver[1,10]
    scalar __fallback = __solver[1,11]
    scalar __warnings = __solver[1,12]
    scalar __preq = __solver[1,13]
    scalar __pused = __solver[1,14]
    scalar __pilot_iters = __solver[1,15]
    scalar __pilot_contract = __solver[1,16]
    scalar __res_used = __solver[2,2]
    scalar __res_iters = __solver[2,3]
    scalar __res_fallback = __solver[2,11]
    scalar __res_pilot_iters = __solver[2,15]
    scalar __res_pilot_contract = __solver[2,16]
    scalar __native = __runtime[20,2]
    scalar __precommit = __runtime[21,2]
    scalar __effective = __runtime[29,2]
    scalar __reported_tol = __runtime[23,2]
    if rowsof(__fe) >= 1 scalar __level1 = __fe[1,4]
    if rowsof(__fe) >= 2 scalar __level2 = __fe[2,4]
    if rowsof(__fe) >= 3 scalar __level3 = __fe[3,4]
    if rowsof(__dof) >= 2 scalar __graph_components = __dof[2,2]
    scalar __rss = e(rss)
    capture scalar __b1 = _b[x1]
    capture scalar __b2 = _b[x2]
    capture scalar __b3 = _b[x3]
    capture scalar __b4 = _b[x4]
}
local __warning = cond(__fallback == 1 & __req == 1 & __used == 4 & __pilot_contract >= .008, "auto_lsmr_failed_then_cg", cond(__fallback == 1 & __req == 7 & __used == 4, "cmg_failed_then_cg", cond(__warnings > 0, "solver_warning", "")))
post __runs ("bal_p3_r20_m002_s1") ("balanced_diagnostic") ("balanced_worker_firm") ("periods3_ratio2_mobility0.02_seed1") (26228582) (24000) (3) (8000) (4000) (0) (2) (0.02) (4) (5) (8) ("worker firm") (__graph_unique_edges) (__graph_duplicate_fraction) (__graph_edges_per_vertex) (__graph_worker_degree_mean) (__graph_worker_degree_p50) (__graph_worker_degree_p90) (__graph_worker_degree_max) (__graph_firm_degree_mean) (__graph_firm_degree_p50) (__graph_firm_degree_p90) (__graph_firm_degree_max) (3) (4) ("cmg") ("cmg") ("off") (1e-08) (__rc) (__seconds) (__req) (__used) (__iterations) (__stop) (__certified) (__absinf) (__relinf) (__rel2) (__normal) (__setup) (__solve) (__fallback) (__warnings) (__preq) (__pused) (__pilot_iters) (__pilot_contract) (__res_used) (__res_iters) (__res_fallback) (__res_pilot_iters) (__res_pilot_contract) (__native) (__precommit) (__effective) (__reported_tol) (__level1) (__level2) (__level3) (__rss) (__b1) (__b2) (__b3) (__b4) (__graph_components) (`"`__warning'"')
timer clear 91
timer on 91
capture quietly fereg y x1 x2 x3 x4, absorb(worker firm) dof(pairwise) solver(auto) threads(8) tolerance(1e-08) iterate(16000)
scalar __rc = _rc
timer off 91
quietly timer list 91
scalar __seconds = r(t91)
timer clear 91
foreach __name in req used iterations stop certified absinf relinf rel2 normal setup solve fallback warnings preq pused pilot_iters pilot_contract res_used res_iters res_fallback res_pilot_iters res_pilot_contract native precommit effective reported_tol level1 level2 level3 rss b1 b2 b3 b4 graph_components {
    scalar __`__name' = .
}
if __rc == 0 {
    matrix __solver = e(fereg_solver)
    matrix __runtime = e(fereg_runtime)
    matrix __fe = e(fereg_fe)
    matrix __dof = e(dof_table)
    scalar __req = __solver[1,1]
    scalar __used = __solver[1,2]
    scalar __iterations = __solver[1,3]
    scalar __stop = __solver[1,4]
    scalar __certified = inlist(__stop,1,2)
    scalar __absinf = __solver[1,5]
    scalar __relinf = __solver[1,6]
    scalar __rel2 = __solver[1,7]
    scalar __normal = __solver[1,8]
    scalar __setup = __solver[1,9]
    scalar __solve = __solver[1,10]
    scalar __fallback = __solver[1,11]
    scalar __warnings = __solver[1,12]
    scalar __preq = __solver[1,13]
    scalar __pused = __solver[1,14]
    scalar __pilot_iters = __solver[1,15]
    scalar __pilot_contract = __solver[1,16]
    scalar __res_used = __solver[2,2]
    scalar __res_iters = __solver[2,3]
    scalar __res_fallback = __solver[2,11]
    scalar __res_pilot_iters = __solver[2,15]
    scalar __res_pilot_contract = __solver[2,16]
    scalar __native = __runtime[20,2]
    scalar __precommit = __runtime[21,2]
    scalar __effective = __runtime[29,2]
    scalar __reported_tol = __runtime[23,2]
    if rowsof(__fe) >= 1 scalar __level1 = __fe[1,4]
    if rowsof(__fe) >= 2 scalar __level2 = __fe[2,4]
    if rowsof(__fe) >= 3 scalar __level3 = __fe[3,4]
    if rowsof(__dof) >= 2 scalar __graph_components = __dof[2,2]
    scalar __rss = e(rss)
    capture scalar __b1 = _b[x1]
    capture scalar __b2 = _b[x2]
    capture scalar __b3 = _b[x3]
    capture scalar __b4 = _b[x4]
}
local __warning = cond(__fallback == 1 & __req == 1 & __used == 4 & __pilot_contract >= .008, "auto_lsmr_failed_then_cg", cond(__fallback == 1 & __req == 7 & __used == 4, "cmg_failed_then_cg", cond(__warnings > 0, "solver_warning", "")))
post __runs ("bal_p3_r20_m002_s1") ("balanced_diagnostic") ("balanced_worker_firm") ("periods3_ratio2_mobility0.02_seed1") (26228582) (24000) (3) (8000) (4000) (0) (2) (0.02) (4) (5) (8) ("worker firm") (__graph_unique_edges) (__graph_duplicate_fraction) (__graph_edges_per_vertex) (__graph_worker_degree_mean) (__graph_worker_degree_p50) (__graph_worker_degree_p90) (__graph_worker_degree_max) (__graph_firm_degree_mean) (__graph_firm_degree_p50) (__graph_firm_degree_p90) (__graph_firm_degree_max) (3) (5) ("auto") ("auto") ("none") (1e-08) (__rc) (__seconds) (__req) (__used) (__iterations) (__stop) (__certified) (__absinf) (__relinf) (__rel2) (__normal) (__setup) (__solve) (__fallback) (__warnings) (__preq) (__pused) (__pilot_iters) (__pilot_contract) (__res_used) (__res_iters) (__res_fallback) (__res_pilot_iters) (__res_pilot_contract) (__native) (__precommit) (__effective) (__reported_tol) (__level1) (__level2) (__level3) (__rss) (__b1) (__b2) (__b3) (__b4) (__graph_components) (`"`__warning'"')
timer clear 91
timer on 91
capture quietly fereg y x1 x2 x3 x4, absorb(worker firm) dof(pairwise) solver(map) threads(8) tolerance(1e-08) iterate(16000)
scalar __rc = _rc
timer off 91
quietly timer list 91
scalar __seconds = r(t91)
timer clear 91
foreach __name in req used iterations stop certified absinf relinf rel2 normal setup solve fallback warnings preq pused pilot_iters pilot_contract res_used res_iters res_fallback res_pilot_iters res_pilot_contract native precommit effective reported_tol level1 level2 level3 rss b1 b2 b3 b4 graph_components {
    scalar __`__name' = .
}
if __rc == 0 {
    matrix __solver = e(fereg_solver)
    matrix __runtime = e(fereg_runtime)
    matrix __fe = e(fereg_fe)
    matrix __dof = e(dof_table)
    scalar __req = __solver[1,1]
    scalar __used = __solver[1,2]
    scalar __iterations = __solver[1,3]
    scalar __stop = __solver[1,4]
    scalar __certified = inlist(__stop,1,2)
    scalar __absinf = __solver[1,5]
    scalar __relinf = __solver[1,6]
    scalar __rel2 = __solver[1,7]
    scalar __normal = __solver[1,8]
    scalar __setup = __solver[1,9]
    scalar __solve = __solver[1,10]
    scalar __fallback = __solver[1,11]
    scalar __warnings = __solver[1,12]
    scalar __preq = __solver[1,13]
    scalar __pused = __solver[1,14]
    scalar __pilot_iters = __solver[1,15]
    scalar __pilot_contract = __solver[1,16]
    scalar __res_used = __solver[2,2]
    scalar __res_iters = __solver[2,3]
    scalar __res_fallback = __solver[2,11]
    scalar __res_pilot_iters = __solver[2,15]
    scalar __res_pilot_contract = __solver[2,16]
    scalar __native = __runtime[20,2]
    scalar __precommit = __runtime[21,2]
    scalar __effective = __runtime[29,2]
    scalar __reported_tol = __runtime[23,2]
    if rowsof(__fe) >= 1 scalar __level1 = __fe[1,4]
    if rowsof(__fe) >= 2 scalar __level2 = __fe[2,4]
    if rowsof(__fe) >= 3 scalar __level3 = __fe[3,4]
    if rowsof(__dof) >= 2 scalar __graph_components = __dof[2,2]
    scalar __rss = e(rss)
    capture scalar __b1 = _b[x1]
    capture scalar __b2 = _b[x2]
    capture scalar __b3 = _b[x3]
    capture scalar __b4 = _b[x4]
}
local __warning = cond(__fallback == 1 & __req == 1 & __used == 4 & __pilot_contract >= .008, "auto_lsmr_failed_then_cg", cond(__fallback == 1 & __req == 7 & __used == 4, "cmg_failed_then_cg", cond(__warnings > 0, "solver_warning", "")))
post __runs ("bal_p3_r20_m002_s1") ("balanced_diagnostic") ("balanced_worker_firm") ("periods3_ratio2_mobility0.02_seed1") (26228582) (24000) (3) (8000) (4000) (0) (2) (0.02) (4) (5) (8) ("worker firm") (__graph_unique_edges) (__graph_duplicate_fraction) (__graph_edges_per_vertex) (__graph_worker_degree_mean) (__graph_worker_degree_p50) (__graph_worker_degree_p90) (__graph_worker_degree_max) (__graph_firm_degree_mean) (__graph_firm_degree_p50) (__graph_firm_degree_p90) (__graph_firm_degree_max) (3) (6) ("map") ("map") ("none") (1e-08) (__rc) (__seconds) (__req) (__used) (__iterations) (__stop) (__certified) (__absinf) (__relinf) (__rel2) (__normal) (__setup) (__solve) (__fallback) (__warnings) (__preq) (__pused) (__pilot_iters) (__pilot_contract) (__res_used) (__res_iters) (__res_fallback) (__res_pilot_iters) (__res_pilot_contract) (__native) (__precommit) (__effective) (__reported_tol) (__level1) (__level2) (__level3) (__rss) (__b1) (__b2) (__b3) (__b4) (__graph_components) (`"`__warning'"')
postclose __runs
use "/Users/johannes/Git/fereg/benchmarks/reports/development/solver-routing-balanced-diagnostic-smoke-v1/work/bal_p3_r20_m002_s1_runs.dta", clear
export delimited using "/Users/johannes/Git/fereg/benchmarks/reports/development/solver-routing-balanced-diagnostic-smoke-v1/work/bal_p3_r20_m002_s1_runs.csv", replace
display as result "CALIBRATION_CASE|case=bal_p3_r20_m002_s1|rows=" _N "|rc=0"
exit 0
