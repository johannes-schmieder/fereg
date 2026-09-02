/*
    Compare the public Veneto worker--firm panel with matched sparse and
    dense synthetic designs, then time the estimators used by the other manual
    OLS benchmarks on the largest connected set of the observed data.

    The synthetic panels match Veneto's worker count, firm count, and two
    observations per worker.  They differ only in the probability of changing
    firms between observations: 2% for sparse and 20% for dense, as in
    manual_benchmarks.do.  Transition mobility is therefore the primary
    dense-versus-sparse diagnostic.  Graph-component and degree summaries are
    reported separately rather than combined through an arbitrary score.

    By default, the file downloads the example bundled with LeaveOutKSS 0.1.0
    from the exact CRAN-mirror commit below.  For an offline run, set

        global FEREG_VENETO_DATA /path/to/test.csv

    before running this file.  The pinned raw file has SHA-256
    9b6971d8364d0ae617b1dd7d8b06351a3f0b322e97e1f2e65a5f96ccd654f47c.
    Its newline-normalized SHA-256 is
    93e57a413a8cfccdcb043c5d793105a67b2dc9ebd27d5d3a4f1800abf89a2241.

    This is a diagnostic manual benchmark, not release evidence.
*/

version 19.0
clear all
set more off
set varabbrev off
set linesize 255

/* ---------------------------- User settings ---------------------------- */
local dense_mobility  .20
local sparse_mobility .02
local repetitions     5
local seed             20260901
local requested_threads 8
/* ----------------------------------------------------------------------- */

local data_url                                                     ///
    "https://raw.githubusercontent.com/cran/LeaveOutKSS/318838d4d51300d0887d26559dad10c8af3b0365/inst/extdata/test.csv"
local data_file `"$FEREG_VENETO_DATA"'
if strtrim(`"`data_file'"') == "" {
    tempfile downloaded_veneto
    capture noisily copy `"`data_url'"' `"`downloaded_veneto'"', replace
    if _rc {
        display as error "could not download the pinned Veneto example"
        display as error ///
            "set global FEREG_VENETO_DATA to a local copy of LeaveOutKSS inst/extdata/test.csv"
        exit 601
    }
    local data_file `"`downloaded_veneto'"'
}
confirm file `"`data_file'"'

local processor_cap = min(c(processors_lic), c(processors_mach))
local threads = min(`requested_threads', `processor_cap')
if `repetitions' < 1 | `repetitions' != floor(`repetitions') {
    display as error "repetitions must be a positive integer"
    exit 198
}
if !inrange(`dense_mobility', 0, 1) |                              ///
   !inrange(`sparse_mobility', 0, 1) |                             ///
   `dense_mobility' <= `sparse_mobility' {
    display as error "mobility probabilities must satisfy 0 <= sparse < dense <= 1"
    exit 198
}

adopath + "./stata"
adopath + "./dist"
capture which fereg
if _rc {
    display as error "fereg is not installed or not on the current adopath"
    exit 111
}
foreach dependency in reghdfe greg gregress {
    capture which `dependency'
    if _rc {
        display as error "`dependency' is required for this manual comparison"
        exit 111
    }
}

capture mata: mata drop __fven_component_stats()
capture mata: mata drop __fven_mark_largest_component()
mata:
void __fven_component_stats(string scalar worker_name,
                            string scalar firm_name)
{
    real colvector worker, firm, parent, tree_size, roots
    real scalar workers, firms, vertices, observation, left, right
    real scalar temporary, components, largest_vertices, largest_root
    real scalar largest_workers, largest_firms

    worker = st_data(., worker_name)
    firm = st_data(., firm_name)
    workers = max(worker)
    firms = max(firm)
    vertices = workers + firms
    parent = 1::vertices
    tree_size = J(vertices, 1, 1)

    for (observation = 1; observation <= rows(worker); observation++) {
        left = worker[observation]
        right = workers + firm[observation]
        while (parent[left] != left) left = parent[left]
        while (parent[right] != right) right = parent[right]
        if (left != right) {
            if (tree_size[left] < tree_size[right]) {
                temporary = left
                left = right
                right = temporary
            }
            parent[right] = left
            tree_size[left] = tree_size[left] + tree_size[right]
        }
    }

    roots = selectindex(parent :== (1::vertices))
    components = rows(roots)
    largest_vertices = max(tree_size[roots])
    largest_root = roots[selectindex(tree_size[roots] :== largest_vertices)[1]]
    largest_workers = 0
    largest_firms = 0

    for (left = 1; left <= workers; left++) {
        right = left
        while (parent[right] != right) right = parent[right]
        if (right == largest_root) largest_workers = largest_workers + 1
    }
    for (left = 1; left <= firms; left++) {
        right = workers + left
        while (parent[right] != right) right = parent[right]
        if (right == largest_root) largest_firms = largest_firms + 1
    }

    st_numscalar("__fven_components", components)
    st_numscalar("__fven_largest_vertices", largest_vertices)
    st_numscalar("__fven_largest_workers", largest_workers)
    st_numscalar("__fven_largest_firms", largest_firms)
}

void __fven_mark_largest_component(string scalar worker_name,
                                    string scalar firm_name,
                                    string scalar marker_name)
{
    real colvector worker, firm, parent, tree_size, roots, marker
    real scalar workers, firms, vertices, observation, left, right
    real scalar temporary, largest_vertices, largest_root

    worker = st_data(., worker_name)
    firm = st_data(., firm_name)
    workers = max(worker)
    firms = max(firm)
    vertices = workers + firms
    parent = 1::vertices
    tree_size = J(vertices, 1, 1)

    for (observation = 1; observation <= rows(worker); observation++) {
        left = worker[observation]
        right = workers + firm[observation]
        while (parent[left] != left) left = parent[left]
        while (parent[right] != right) right = parent[right]
        if (left != right) {
            if (tree_size[left] < tree_size[right]) {
                temporary = left
                left = right
                right = temporary
            }
            parent[right] = left
            tree_size[left] = tree_size[left] + tree_size[right]
        }
    }

    roots = selectindex(parent :== (1::vertices))
    largest_vertices = max(tree_size[roots])
    largest_root = roots[selectindex(tree_size[roots] :== largest_vertices)[1]]
    marker = J(rows(worker), 1, 0)
    for (observation = 1; observation <= rows(worker); observation++) {
        left = worker[observation]
        while (parent[left] != left) left = parent[left]
        marker[observation] = left == largest_root
    }
    st_store(., marker_name, marker)
}
end

capture program drop __fven_topology
program define __fven_topology, rclass
    version 19.0
    syntax, Worker(varname numeric) Firm(varname numeric) Time(varname numeric)

    preserve
    keep `worker' `firm' `time'
    drop if missing(`worker', `firm', `time')
    sort `worker' `time'

    tempvar worker_tag firm_tag match_tag transition move              ///
        worker_firm_first firm_worker_first worker_degree firm_degree ///
        worker_code firm_code

    egen byte `worker_tag' = tag(`worker')
    egen byte `firm_tag' = tag(`firm')
    egen byte `match_tag' = tag(`worker' `firm')
    by `worker' (`time'): generate byte `transition' = _n > 1
    by `worker' (`time'): generate byte `move' =                       ///
        `transition' & `firm' != `firm'[_n-1]

    quietly count
    local observations = r(N)
    quietly count if `worker_tag'
    local workers = r(N)
    quietly count if `firm_tag'
    local firms = r(N)
    quietly count if `match_tag'
    local matches = r(N)
    quietly count if `transition'
    local transitions = r(N)
    quietly count if `move'
    local moves = r(N)
    local mobility_rate = `moves' / `transitions'

    bysort `worker' `firm': generate byte `worker_firm_first' = _n == 1
    bysort `worker': egen int `worker_degree' = total(`worker_firm_first')
    quietly count if `worker_tag' & `worker_degree' > 1
    local multifirm_worker_share = r(N) / `workers'
    quietly summarize `worker_degree' if `worker_tag', detail
    local worker_degree_mean = r(mean)
    local worker_degree_p50 = r(p50)

    bysort `firm' `worker': generate byte `firm_worker_first' = _n == 1
    bysort `firm': egen long `firm_degree' = total(`firm_worker_first')
    quietly summarize `firm_degree' if `firm_tag', detail
    local firm_degree_mean = r(mean)
    local firm_degree_p50 = r(p50)

    egen long `worker_code' = group(`worker')
    egen long `firm_code' = group(`firm')
    mata: __fven_component_stats("`worker_code'", "`firm_code'")
    local components = scalar(__fven_components)
    local largest_vertices = scalar(__fven_largest_vertices)
    local largest_workers = scalar(__fven_largest_workers)
    local largest_firms = scalar(__fven_largest_firms)
    local largest_vertex_share = `largest_vertices' / (`workers' + `firms')
    local components_per_1000_vertices =                             ///
        1000 * `components' / (`workers' + `firms')
    local edge_density = `matches' / (`workers' * `firms')
    restore

    return scalar observations = `observations'
    return scalar workers = `workers'
    return scalar firms = `firms'
    return scalar matches = `matches'
    return scalar transitions = `transitions'
    return scalar moves = `moves'
    return scalar mobility_rate = `mobility_rate'
    return scalar multifirm_worker_share = `multifirm_worker_share'
    return scalar worker_degree_mean = `worker_degree_mean'
    return scalar worker_degree_p50 = `worker_degree_p50'
    return scalar firm_degree_mean = `firm_degree_mean'
    return scalar firm_degree_p50 = `firm_degree_p50'
    return scalar components = `components'
    return scalar largest_vertices = `largest_vertices'
    return scalar largest_workers = `largest_workers'
    return scalar largest_firms = `largest_firms'
    return scalar largest_vertex_share = `largest_vertex_share'
    return scalar components_per_1000_vertices =                     ///
        `components_per_1000_vertices'
    return scalar edge_density = `edge_density'
end

capture program drop __fven_generate_synthetic
program define __fven_generate_synthetic
    version 19.0
    syntax, Workers(integer) Firms(integer) Mobility(real) Seed(integer)

    clear
    set rng mt64
    set seed `seed'
    set obs `workers'
    generate long worker = _n
    expand 2
    bysort worker: generate int year = _n
    bysort worker (year): generate long firm =                       ///
        mod(7919*worker - 1, `firms') + 1 if _n == 1
    generate long __fven_candidate =                                 ///
        mod(104729*worker + 7919*year - 1, `firms') + 1 if year == 2
    bysort worker (year): replace __fven_candidate =                 ///
        mod(__fven_candidate, `firms') + 1                           ///
        if year == 2 & __fven_candidate == firm[_n-1]
    bysort worker (year): replace firm =                             ///
        cond(runiform() < `mobility', __fven_candidate, firm[_n-1]) ///
        if year == 2
    drop __fven_candidate
    sort worker year
end

/* Load and validate the public panel. */
import delimited using `"`data_file'"', varnames(nonames) clear
assert c(k) == 10
rename v1 worker
rename v2 firm
rename v3 year
rename v4 outcome
keep worker firm year outcome
confirm numeric variable worker firm year outcome
assert _N == 71614
isid worker year
sort worker year
tempfile veneto
save `veneto', replace

tempname topology
matrix `topology' = J(3, 13, .)
matrix rownames `topology' = Veneto Dense_20pct Sparse_2pct
matrix colnames `topology' = observations workers firms matches       ///
    transitions moves mobility multiworker worker_degree firm_degree ///
    components largest_share components_per_1000

quietly __fven_topology, worker(worker) firm(firm) time(year)
local veneto_workers = r(workers)
local veneto_firms = r(firms)
local veneto_mobility = r(mobility_rate)
local veneto_largest_share = r(largest_vertex_share)
local veneto_components_per_1000 = r(components_per_1000_vertices)
matrix `topology'[1,1]  = r(observations)
matrix `topology'[1,2]  = r(workers)
matrix `topology'[1,3]  = r(firms)
matrix `topology'[1,4]  = r(matches)
matrix `topology'[1,5]  = r(transitions)
matrix `topology'[1,6]  = r(moves)
matrix `topology'[1,7]  = r(mobility_rate)
matrix `topology'[1,8]  = r(multifirm_worker_share)
matrix `topology'[1,9]  = r(worker_degree_mean)
matrix `topology'[1,10] = r(firm_degree_mean)
matrix `topology'[1,11] = r(components)
matrix `topology'[1,12] = r(largest_vertex_share)
matrix `topology'[1,13] = r(components_per_1000_vertices)

quietly __fven_generate_synthetic, workers(`veneto_workers')         ///
    firms(`veneto_firms') mobility(`dense_mobility') seed(`seed')
quietly __fven_topology, worker(worker) firm(firm) time(year)
local dense_realized_mobility = r(mobility_rate)
local dense_largest_share = r(largest_vertex_share)
local dense_components_per_1000 = r(components_per_1000_vertices)
forvalues column = 1/13 {
    local result_name : word `column' of                             ///
        observations workers firms matches transitions moves        ///
        mobility_rate multifirm_worker_share worker_degree_mean      ///
        firm_degree_mean components largest_vertex_share             ///
        components_per_1000_vertices
    matrix `topology'[2,`column'] = r(`result_name')
}

quietly __fven_generate_synthetic, workers(`veneto_workers')         ///
    firms(`veneto_firms') mobility(`sparse_mobility')                ///
    seed(`=`seed' + 1')
quietly __fven_topology, worker(worker) firm(firm) time(year)
local sparse_realized_mobility = r(mobility_rate)
local sparse_largest_share = r(largest_vertex_share)
local sparse_components_per_1000 = r(components_per_1000_vertices)
forvalues column = 1/13 {
    local result_name : word `column' of                             ///
        observations workers firms matches transitions moves        ///
        mobility_rate multifirm_worker_share worker_degree_mean      ///
        firm_degree_mean components largest_vertex_share             ///
        components_per_1000_vertices
    matrix `topology'[3,`column'] = r(`result_name')
}

local dense_mobility_distance =                                    ///
    abs(`veneto_mobility' - `dense_realized_mobility')
local sparse_mobility_distance =                                   ///
    abs(`veneto_mobility' - `sparse_realized_mobility')
local mobility_classification = cond(                               ///
    `dense_mobility_distance' < `sparse_mobility_distance',         ///
    "dense-like", "sparse-like")

display as text _newline "Veneto topology versus matched synthetic panels"
matrix list `topology', format(%12.6g)
display as text "Primary classification by transition mobility: "     ///
    as result "`mobility_classification'"
display as text "  Veneto mobility:       " as result %9.4f `veneto_mobility'
display as text "  Dense realized rate:   " as result %9.4f `dense_realized_mobility'
display as text "  Sparse realized rate:  " as result %9.4f `sparse_realized_mobility'
display as text "  Veneto largest-component vertex share: "             ///
    as result %9.4f `veneto_largest_share'
display as text "  Dense largest-component vertex share:  "             ///
    as result %9.4f `dense_largest_share'
display as text "  Sparse largest-component vertex share: "             ///
    as result %9.4f `sparse_largest_share'
display as text "  Veneto components per 1,000 vertices:  "             ///
    as result %9.4f `veneto_components_per_1000'
display as text "  Dense components per 1,000 vertices:   "             ///
    as result %9.4f `dense_components_per_1000'
display as text "  Sparse components per 1,000 vertices:  "             ///
    as result %9.4f `sparse_components_per_1000'

/* Warm and time the observed-data estimators on the largest connected set. */
use `veneto', clear
local original_processors = c(processors)
quietly set processors `threads'
global GTOOLS_BETA "I KNOW WHAT I AM DOING"

/* Freeze one post-singleton sample so command timings are comparable. */
quietly fereg outcome, absorb(worker firm) dof(pairwise)             ///
    threads(`threads')
generate byte __fven_common_sample = e(sample)
quietly count if __fven_common_sample
local estimation_n = r(N)
assert `estimation_n' > 0

keep if __fven_common_sample
tempvar worker_code firm_code largest_connected
egen long `worker_code' = group(worker)
egen long `firm_code' = group(firm)
generate byte `largest_connected' = 0
mata: __fven_mark_largest_component("`worker_code'", "`firm_code'", "`largest_connected'")
quietly count if `largest_connected'
local largest_connected_n = r(N)
assert `largest_connected_n' > 0
keep if `largest_connected'
drop __fven_common_sample `worker_code' `firm_code' `largest_connected'

quietly __fven_topology, worker(worker) firm(firm) time(year)
local largest_connected_workers = r(workers)
local largest_connected_firms = r(firms)
local largest_connected_components = r(components)
assert `largest_connected_components' == 1

local command_fereg_auto ///
    "fereg outcome, absorb(worker firm) dof(pairwise) threads(`threads')"
local command_fereg_cmg ///
    "fereg outcome, absorb(worker firm) dof(pairwise) threads(`threads') solver(CMG)"
local command_reghdfe ///
    "reghdfe outcome, absorb(worker firm) dof(pairwise)"
local command_greg    ///
    "greg outcome, absorb(worker firm) mata(GtoolsRegress)"
local command_areg    ///
    "areg outcome, absorb(worker firm) dfabsorb noabsorbtest nolog"

foreach arm in fereg_auto fereg_cmg reghdfe greg areg {
    capture quietly `command_`arm''
    if _rc {
        local warm_rc = _rc
        display as error "`arm' warm-up failed with r(`warm_rc')"
        exit `warm_rc'
    }
    if e(N) != `largest_connected_n' {
        display as error ///
            "`arm' used " e(N) " observations; expected `largest_connected_n'"
        exit 459
    }
}

timer clear
forvalues repetition = 1/`repetitions' {
    local rotation = mod(`repetition' - 1, 5)
    if `rotation' == 0 local command_order ///
        "fereg_auto fereg_cmg reghdfe greg areg"
    if `rotation' == 1 local command_order ///
        "fereg_cmg reghdfe greg areg fereg_auto"
    if `rotation' == 2 local command_order ///
        "reghdfe greg areg fereg_auto fereg_cmg"
    if `rotation' == 3 local command_order ///
        "greg areg fereg_auto fereg_cmg reghdfe"
    if `rotation' == 4 local command_order ///
        "areg fereg_auto fereg_cmg reghdfe greg"
    foreach arm of local command_order {
        local timer_id = cond("`arm'" == "fereg_auto", 1,             ///
            cond("`arm'" == "fereg_cmg", 2,                           ///
            cond("`arm'" == "reghdfe", 3,                            ///
            cond("`arm'" == "greg", 4, 5))))
        timer on `timer_id'
        capture quietly `command_`arm''
        local run_rc = _rc
        timer off `timer_id'
        if `run_rc' {
            display as error ///
                "`arm' failed in repetition `repetition' with r(`run_rc')"
            exit `run_rc'
        }
    }
}
quietly timer list

tempname runtime
matrix `runtime' = (r(t1)/`repetitions' \                        ///
                    r(t2)/`repetitions' \                        ///
                    r(t3)/`repetitions' \                        ///
                    r(t4)/`repetitions' \                        ///
                    r(t5)/`repetitions')
matrix rownames `runtime' = fereg_auto fereg_cmg reghdfe greg areg
matrix colnames `runtime' = mean_seconds
display as text _newline "Largest-connected-set Veneto estimator timings"
matrix list `runtime', format(%10.4f)

quietly fereg outcome, absorb(worker firm) dof(pairwise)            ///
    threads(`threads') timeit
local auto_solver_code = e(fereg_solver)[1,2]
local auto_iterations = e(fereg_solver)[1,3]
local auto_fallback = e(fereg_solver)[1,11]
local auto_solver_name = cond(`auto_solver_code' == 2, "MAP",       ///
    cond(`auto_solver_code' == 3, "LSMR",                           ///
    cond(`auto_solver_code' == 4, "CG",                             ///
    cond(`auto_solver_code' == 5, "Irons-Tuck",                     ///
    cond(`auto_solver_code' == 6, "SQUAREM",                        ///
    cond(`auto_solver_code' == 7, "CMG", "unknown"))))))
display as text "Common post-singleton estimation sample: "         ///
    as result %9.0fc `estimation_n'
display as text "Largest connected set: "                           ///
    as result %9.0fc `largest_connected_n'                           ///
    as text " observations; " as result %9.0fc `largest_connected_workers' ///
    as text " workers; " as result %9.0fc `largest_connected_firms' ///
    as text " firms"
display as text "fereg auto route on largest connected set: "       ///
    as result "`auto_solver_name' (code `auto_solver_code'; iterations `auto_iterations'; fallback `auto_fallback')"
matrix list e(fereg_solver)
matrix list e(fereg_runtime)

quietly fereg outcome, absorb(worker firm) dof(pairwise)             ///
    threads(`threads') solver(CMG) timeit
local cmg_solver_code = e(fereg_solver)[1,2]
local cmg_iterations = e(fereg_solver)[1,3]
local cmg_fallback = e(fereg_solver)[1,11]
local cmg_solver_name = cond(`cmg_solver_code' == 7, "CMG",         ///
    cond(`cmg_solver_code' == 4, "CG", "other"))
display as text "fereg explicit CMG route on largest connected set: " ///
    as result "`cmg_solver_name' (code `cmg_solver_code'; iterations `cmg_iterations'; fallback `cmg_fallback')"
matrix list e(fereg_solver)
matrix list e(fereg_runtime)
set processors `original_processors'

display as result ///
    "FEREG_SIMPLE_VENETO|PASS|classification=`mobility_classification'|N=71614|post_singleton_N=`estimation_n'|largest_connected_N=`largest_connected_n'|auto_used=`auto_solver_name'|cmg_used=`cmg_solver_name'|repetitions=`repetitions'"
