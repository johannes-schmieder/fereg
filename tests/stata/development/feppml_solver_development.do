version 18.0
clear all
set more off
set linesize 255

local mode : environment FEREG_PPML_SOLVER_MODE
local repository : environment FEREG_PPML_SOLVER_REPOSITORY
local dataset : environment FEREG_PPML_SOLVER_DATASET
local result : environment FEREG_PPML_SOLVER_RESULT
local data_csv : environment FEREG_PPML_SOLVER_DATA_CSV
local data_meta : environment FEREG_PPML_SOLVER_DATA_META
local topology : environment FEREG_PPML_SOLVER_TOPOLOGY
local difficulty : environment FEREG_PPML_SOLVER_DIFFICULTY
local arm : environment FEREG_PPML_SOLVER_ARM
local reference_root : environment FEREG_PPML_SOLVER_REFERENCE_ROOT
local stata_log : environment FEREG_PPML_SOLVER_STATA_LOG
local n = real(`"`: environment FEREG_PPML_SOLVER_N'"')
local controls = real(`"`: environment FEREG_PPML_SOLVER_K'"')
local threads = real(`"`: environment FEREG_PPML_SOLVER_THREADS'"')
local repetitions = real(`"`: environment FEREG_PPML_SOLVER_REPETITIONS'"')
local seed = real(`"`: environment FEREG_PPML_SOLVER_SEED'"')
if strtrim(`"`stata_log'"') != "" log using `"`stata_log'"', text replace

if `"`mode'"' == "generate" {
    if missing(`n') | `n' <= 0 | missing(`controls') | `controls' < 0 | missing(`seed') {
        display as error "invalid PPML solver-development generation dimensions"
        exit 198
    }
    set rng mt64
    set seed `seed'
    set obs `n'
    generate long observation = _n

    local long = (`"`difficulty'"' == "long")
    local levels1 = cond(`long', min(5000, floor(`n'/3)), 80)
    local levels2 = cond(`long', min(4000, floor(`n'/4)), 64)
    local levels3 = cond(`long', min(2500, floor(`n'/5)), 40)
    if `n' <= 1000 {
        local levels1 12
        local levels2 10
        local levels3 8
    }
    if `"`topology'"' == "dense_2fe" {
        local levels1 20
        local levels2 16
    }
    if `"`topology'"' == "many_levels_2fe" {
        local levels1 = floor(`n'/3)
        local levels2 = floor(`n'/4)
    }

    generate long g1 = mod(_n - 1, `levels1') + 1
    generate long g2 = mod(floor((_n - 1) / max(1, `levels1')) + 7 * (_n - 1), `levels2') + 1
    if inlist(`"`topology'"', "chain_2fe", "sparse_3fe") {
        replace g2 = mod(g1 - 1 + mod(floor((_n - 1) / max(1, `levels1')), 2), `levels2') + 1
    }
    if `"`topology'"' == "hub_2fe" {
        replace g1 = 1 if mod(_n, 3) != 0
        replace g2 = mod(13 * (_n - 1), `levels2') + 1
    }
    if `"`topology'"' == "skewed_2fe" {
        replace g1 = 1 + floor((`levels1' - 1) * (runiform()^3))
        replace g2 = 1 + floor((`levels2' - 1) * (runiform()^2))
    }
    if inlist(`"`topology'"', "balanced_3fe", "sparse_3fe") {
        generate long g3 = mod(11 * (_n - 1) + floor((_n - 1) / max(1, `levels2')), `levels3') + 1
    }

    local control_list
    if `controls' > 0 {
        forvalues index = 1/`controls' {
            generate double x`index' = rnormal() + ///
                .20 * sin((_n + 3 * `index') / (11 + `index')) + ///
                .10 * cos((_n * (1 + mod(`index', 7))) / (29 + `index'))
            local control_list `control_list' x`index'
        }
    }
    generate double eta_data = .15 * sin(g1 / 7) - .12 * cos(g2 / 5)
    capture confirm variable g3
    if !_rc replace eta_data = eta_data + .08 * sin(g3 / 3)
    if `controls' > 0 replace eta_data = eta_data + .06 * x1
    if `controls' > 1 replace eta_data = eta_data - .04 * x2
    generate long y = 1 + mod(floor(100 * exp(eta_data) + mod(_n, 17)), 19)
    assert y > 0 & y < .

    quietly egen byte __tag1 = tag(g1)
    quietly count if __tag1
    local observed_levels1 = r(N)
    drop __tag1
    quietly egen byte __tag2 = tag(g2)
    quietly count if __tag2
    local observed_levels2 = r(N)
    drop __tag2
    local observed_levels3 0
    capture confirm variable g3
    if !_rc {
        quietly egen byte __tag3 = tag(g3)
        quietly count if __tag3
        local observed_levels3 = r(N)
        drop __tag3
    }
    file open metadata using `"`data_meta'"', write text replace
    file write metadata "N" _tab "levels1" _tab "levels2" _tab "levels3" _n
    file write metadata %18.0f (_N) _tab %18.0f (`observed_levels1') _tab ///
        %18.0f (`observed_levels2') _tab %18.0f (`observed_levels3') _n
    file close metadata

    compress
    sort observation
    export delimited y `control_list' g* using `"`data_csv'"', replace
    save `"`dataset'"', replace
    display as text "FEREG_PPML_SOLVER_DATA_PASS|N=" _N "|K=`controls'|topology=`topology'|difficulty=`difficulty'"
    exit 0
}

if `"`mode'"' == "prepare-reference" | ///
        (`"`mode'"' == "run" & inlist(`"`arm'"', "ppmlhdfe", "ppmlhdfe_lsmr")) {
    adopath ++ `"`reference_root'/require/src"'
    adopath ++ `"`reference_root'/ftools/src"'
    adopath ++ `"`reference_root'/reghdfe/src"'
    adopath ++ `"`reference_root'/ppmlhdfe/src"'
}

if `"`mode'"' == "prepare-reference" {
    foreach package in require ftools reghdfe ppmlhdfe {
        capture findfile `package'.ado
        if _rc exit _rc
        local found = subinstr(`"`r(fn)'"', "\", "/", .)
        local expected = subinstr(`"`reference_root'/`package'/src/`package'.ado"', "\", "/", .)
        if `"`found'"' != `"`expected'"' {
            display as error "PPML solver-development comparator escaped its pinned source: `package'"
            exit 603
        }
    }
    local expected_mlib = subinstr(`"`reference_root'/ppmlhdfe/src/l/lftools.mlib"', "\", "/", .)
    capture findfile lftools.mlib
    local found_mlib
    if !_rc local found_mlib = subinstr(`"`r(fn)'"', "\", "/", .)
    if _rc | `"`found_mlib'"' != `"`expected_mlib'"' {
        capture noisily ftools, compile
        if _rc exit _rc
        capture mata: mata mlib index
        if _rc exit _rc
        capture findfile lftools.mlib
        if _rc exit _rc
        local found_mlib = subinstr(`"`r(fn)'"', "\", "/", .)
    }
    if `"`found_mlib'"' != `"`expected_mlib'"' {
        display as error "PPML solver-development comparator did not resolve the private lftools library"
        exit 603
    }
    display as text "FEREG_PPML_SOLVER_REFERENCE_PASS|ppmlhdfe=pinned|lftools=private"
    exit 0
}

if `"`mode'"' != "run" {
    display as error "unknown PPML solver-development mode"
    exit 198
}
if missing(`threads') | `threads' <= 0 | missing(`repetitions') | `repetitions' <= 0 {
    display as error "invalid PPML solver-development execution dimensions"
    exit 198
}

if inlist(`"`arm'"', "map", "lsmr", "cmg") {
    adopath ++ `"`repository'/stata"'
    adopath ++ `"`repository'/dist"'
    capture findfile feppml.ado
    if _rc exit _rc
    local found_feppml = subinstr(`"`r(fn)'"', "\", "/", .)
    local expected_feppml = subinstr(`"`repository'/stata/feppml.ado"', "\", "/", .)
    if `"`found_feppml'"' != `"`expected_feppml'"' {
        display as error "PPML solver-development candidate escaped its captured source"
        exit 603
    }
}
if inlist(`"`arm'"', "ppmlhdfe", "ppmlhdfe_lsmr") {
    foreach package in require ftools reghdfe ppmlhdfe {
        capture findfile `package'.ado
        if _rc exit _rc
        local found = subinstr(`"`r(fn)'"', "\", "/", .)
        local expected = subinstr(`"`reference_root'/`package'/src/`package'.ado"', "\", "/", .)
        if `"`found'"' != `"`expected'"' {
            display as error "PPML solver-development comparator escaped its pinned source: `package'"
            exit 603
        }
    }
    local expected_mlib = subinstr(`"`reference_root'/ppmlhdfe/src/l/lftools.mlib"', "\", "/", .)
    capture findfile lftools.mlib
    if _rc exit _rc
    local found_mlib = subinstr(`"`r(fn)'"', "\", "/", .)
    if `"`found_mlib'"' != `"`expected_mlib'"' {
        display as error "PPML solver-development comparator did not resolve the setup library"
        exit 603
    }
    local stata_processors = min(`threads', c(processors_mach), c(processors_lic))
    quietly set processors `stata_processors'
}
use `"`dataset'"', clear

local control_list
if `controls' > 0 {
    forvalues index = 1/`controls' {
        local control_list `control_list' x`index'
    }
}
local absorb_list "g1 g2"
capture confirm variable g3
if !_rc local absorb_list "g1 g2 g3"

local command
local route_command
if `"`arm'"' == "ppmlhdfe" {
    local command `"ppmlhdfe y `control_list', absorb(`absorb_list') vce(robust) separation(none)"'
    local route_command `"`command' verbose(2)"'
}
else if `"`arm'"' == "ppmlhdfe_lsmr" {
    local command `"ppmlhdfe y `control_list', absorb(`absorb_list') vce(robust) separation(none) accel(lsmr)"'
    local route_command `"`command' verbose(2)"'
}
else if inlist(`"`arm'"', "map", "lsmr", "cmg") {
    local command `"feppml y `control_list', absorb(`absorb_list') vce(robust) separation(none) memory(fast) threads(`threads') solver(`arm')"'
}
else {
    display as error "unknown PPML solver-development arm"
    exit 198
}

local warm_command `"`command'"'
if inlist(`"`arm'"', "ppmlhdfe", "ppmlhdfe_lsmr") local warm_command `"`route_command'"'
capture noisily `warm_command'
local warm_rc = _rc
if `warm_rc' {
    display as error "FEREG_PPML_SOLVER_RUN_FAIL|arm=`arm'|rc=`warm_rc'"
    exit `warm_rc'
}
if inlist(`"`arm'"', "ppmlhdfe", "ppmlhdfe_lsmr") {
    local expected_acceleration "conjugate_gradient"
    if `"`arm'"' == "ppmlhdfe_lsmr" local expected_acceleration "lsmr"
    display as text "FEREG_PPML_SOLVER_REFERENCE_ROUTE_PASS|arm=`arm'|acceleration=`expected_acceleration'"
}

file open output using `"`result'"', write text replace
file write output "arm" _tab "repetition" _tab "rc" _tab "seconds" _tab ///
    "N" _tab "ll" _tab "deviance" _tab "converged" _tab "rank" _tab ///
    "df_a" _tab "df_resid" _tab "inner_iterations" _tab "route_code" _tab ///
    "selected_engine_code" _tab "effective_engine_code" _tab ///
    "fallback_count" _tab "effective_threads" _tab "b_names" _tab "b_values" _tab ///
    "v_names" _tab "v_values" _tab "certificate_values" _tab "runtime_values" _n
forvalues repetition = 1/`repetitions' {
    timer clear 91
    timer on 91
    capture noisily `command'
    local rc = _rc
    timer off 91
    quietly timer list 91
    local seconds = r(t91)
    local sample_n .
    local ll .
    local deviance .
    local converged .
    local rank .
    local df_a .
    local df_resid .
    local inner_iterations .
    local route_code .
    local selected_engine_code .
    local effective_engine_code .
    local fallback_count .
    local effective_threads .
    local b_names
    local b_values
    local v_names
    local v_values
    local certificate_values
    local runtime_values
    if !`rc' {
        local sample_n = e(N)
        capture local ll = e(ll)
        capture local deviance = e(deviance)
        capture local converged = e(converged)
        capture local rank = e(rank)
        capture local df_a = e(df_a)
        capture local df_resid = e(df_resid)
        if _rc capture local df_resid = e(df_r)
        capture local inner_iterations = e(inner_iterations)
        if _rc capture local inner_iterations = e(ic)
        if _rc capture local inner_iterations = e(iterations)
        capture local route_code = e(feppml_solver)[1,4]
        capture local selected_engine_code = e(feppml_solver)[1,3]
        capture local effective_engine_code = e(feppml_solver)[1,5]
        capture local fallback_count = e(feppml_solver_fallback_count)
        capture local effective_threads = e(feppml_threads)
        local b_names : colfullnames e(b)
        mata: st_local("b_values", invtokens(strofreal(st_matrix("e(b)"), "%21.17e")))
        local v_names : colfullnames e(V)
        mata: st_local("v_values", invtokens(strofreal(vec(st_matrix("e(V)")')', "%21.17e")))
        capture matrix __certificate = e(feppml_certificate)
        if !_rc mata: st_local("certificate_values", invtokens(strofreal(vec(st_matrix("__certificate"))', "%21.17e")))
        capture matrix __runtime = e(feppml_runtime)
        if !_rc mata: st_local("runtime_values", invtokens(strofreal(vec(st_matrix("__runtime"))', "%21.17e")))
    }
    file write output `"`arm'"' _tab %9.0f (`repetition') _tab %9.0f (`rc') _tab ///
        %21.17e (`seconds') _tab %21.17e (`sample_n') _tab %21.17e (`ll') _tab ///
        %21.17e (`deviance') _tab %21.17e (`converged') _tab %21.17e (`rank') _tab ///
        %21.17e (`df_a') _tab %21.17e (`df_resid') _tab %21.17e (`inner_iterations') _tab ///
        %21.17e (`route_code') _tab %21.17e (`selected_engine_code') _tab ///
        %21.17e (`effective_engine_code') _tab %21.17e (`fallback_count') _tab ///
        %21.17e (`effective_threads') _tab `"`b_names'"' _tab `"`b_values'"' _tab ///
        `"`v_names'"' _tab `"`v_values'"' _tab `"`certificate_values'"' _tab ///
        `"`runtime_values'"' _n
    if `rc' {
        file close output
        display as error "FEREG_PPML_SOLVER_RUN_FAIL|arm=`arm'|rc=`rc'"
        exit `rc'
    }
}
file close output
display as text "FEREG_PPML_SOLVER_RUN_PASS|arm=`arm'|repetitions=`repetitions'"
