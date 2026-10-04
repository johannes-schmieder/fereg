version 19.0
clear all
set more off
set varabbrev off
set linesize 255
args config
include `"`config'"'
if "`output_mode'" == "" local output_mode = cond(`outputs', "combined", "none")
local processors = min(`threads', `processor_cap', c(processors_lic), c(processors_mach))
if `fixed_host_processors' local processors = min(`processor_cap', c(processors_lic), c(processors_mach))
set processors `processors'
sysdir set PLUS `"`refs'"'
sysdir set PERSONAL `"`output'"'
capture adopath - SITE
capture adopath - "."
adopath ++ `"`refs'"'
adopath ++ `"`source'/stata"'
adopath ++ `"`source'/dist"'
if "`arm'" == "fereg" {
    quietly findfile `plugin_name'
    assert `"`r(fn)'"' == `"`source'/dist/`plugin_name'"'
}
quietly use `"`dataset'"', clear
local xvars
forvalues j = 1/`k' {
    local xvars `xvars' x`j'
}
if "`finite_factor'" == "1" local xvars `xvars' i.factor
local absorb
if `effects' > 0 {
forvalues j = 1/`effects' {
    local effect g`j'
    if `slopes' & `j' == 1 local effect g1##c.slope
    if inlist("`output_mode'", "fe", "combined") local effect saved`j'=`effect'
    local absorb `absorb' `effect'
}
}
local absorption
if `effects' > 0 local absorption absorb(`absorb')
local weight
if "`weights'" == "analytic" local weight [aw=weight]
if "`weights'" == "frequency" local weight [fw=weight]
if "`weights'" == "probability" local weight [pw=weight]
local inference
if "`vce'" == "robust" local inference vce(robust)
if "`vce'" == "cluster" local inference vce(cluster cluster)
if "`vce'" == "multiway" local inference vce(cluster cluster cluster2)
local command `arm'
if "`arm'" == "fereg" & "`model'" == "ppml" local command feppml
local controls
if "`arm'" == "fereg" {
    local controls threads(`threads')
    if `engine_option' local controls `controls' engine(`engine')
    if "`tolerance'" != "" local controls `controls' tolerance(`tolerance')
    if "`solver'" != "auto" local controls `controls' solver(`solver')
    if "`memory'" != "" local controls `controls' memory(`memory')
    if "`separation'" != "" & "`model'" == "ppml" local controls `controls' separation(`separation')
    if "`bias'" != "" local controls `controls' biascorrection(`bias') biasunit(biasunit)
    if "`bias'" == "randomized" local controls `controls' biasprobes(16) biasseed(739391) biasstream(0)
}
local post
if inlist("`output_mode'", "residual", "combined") {
    local post residuals(bench_residual)
    if "`model'" == "ppml" & "`arm'" == "fereg" local post `post' mu(bench_fitted)
}
local command `"`command' y `xvars' `weight', `absorption' `inference' `controls' `post'"'
tempname command_seconds clock_resolution
if "`clock_plugin'" != "" {
    program _bench_clock, plugin using(`"`clock_plugin'"')
    scalar `command_seconds' = .
    scalar `clock_resolution' = .
}
timer clear 91
timer on 91
local clock_rc 0
if "`clock_plugin'" != "" {
    capture quietly plugin call _bench_clock, start
    local clock_rc = _rc
}
capture noisily quietly `command'
local rc = _rc
if !`rc' & "`model'" == "ols" & inlist("`output_mode'", "residual", "combined") {
    capture quietly predict double bench_fitted, xb
    local rc = _rc
}
local estimation_rc = `rc'
if "`clock_plugin'" != "" {
    capture quietly plugin call _bench_clock, stop `command_seconds' `clock_resolution'
    if _rc local clock_rc = _rc
    if scalar(`clock_resolution') <= 0 | scalar(`clock_resolution') > 0.0000005 local clock_rc 498
    if scalar(`command_seconds') < 0 | scalar(`command_seconds') >= . local clock_rc 498
}
if !`rc' & `clock_rc' local rc = `clock_rc'
timer off 91
quietly timer list 91
local seconds = r(t91)
local stata_seconds = `seconds'
if "`clock_plugin'" != "" local seconds = scalar(`command_seconds')
tempname metadata
file open `metadata' using `"`output'/metadata.tsv"', write text replace
file write `metadata' "rc" _tab "`rc'" _n "seconds" _tab %24.17g (`seconds') _n
file write `metadata' "estimation_rc" _tab "`estimation_rc'" _n "clock_rc" _tab "`clock_rc'" _n
file write `metadata' "stata_seconds" _tab %24.17g (`stata_seconds') _n
if "`clock_plugin'" != "" file write `metadata' "clock_resolution_seconds" _tab %24.17g (scalar(`clock_resolution')) _n
file write `metadata' "processors" _tab "`processors'" _n
file write `metadata' "engine" _tab "`engine'" _n
if !`rc' {
    if "`arm'" == "fereg" {
        local actual_engine "`e(fereg_engine)'"
        if "`model'" == "ppml" local actual_engine "`e(feppml_engine)'"
        file write `metadata' "actual_engine" _tab "`actual_engine'" _n
    }
    file write `metadata' "N" _tab %24.17g (e(N)) _n
    tempname coeff
    file open `coeff' using `"`output'/coefficients.csv"', write text replace
    file write `coeff' "name,value" _n
    forvalues j = 1/`k' {
        file write `coeff' "x`j'," %24.17g (_b[x`j']) _n
    }
    file close `coeff'
    if "`arm'" == "fereg" & "`model'" == "ols" {
        file write `metadata' "input_seconds" _tab %24.17g (el(e(fereg_runtime),19,2)) _n
        file write `metadata' "native_seconds" _tab %24.17g (el(e(fereg_runtime),20,2)) _n
        file write `metadata' "total_seconds" _tab %24.17g (el(e(fereg_runtime),21,2)) _n
    }
    if "`arm'" == "fereg" & "`model'" == "ppml" {
        file write `metadata' "total_seconds" _tab %24.17g (el(e(feppml_runtime),1,1)) _n
    }
    if "`arm'" == "fereg" {
        tempname telemetry
        file open `telemetry' using `"`output'/telemetry.tsv"', write text replace
        local matrices : e(matrices)
        foreach name in fereg_runtime fereg_solver fereg_memory feppml_runtime feppml_solver feppml_memory bias_diagnostics {
            if strpos(" `matrices' ", " `name' ") > 0 {
                local rows : rownames e(`name')
                forvalues i = 1/`=rowsof(e(`name'))' {
                    local row : word `i' of `rows'
                    forvalues j = 1/`=colsof(e(`name'))' {
                        file write `telemetry' "`name'.`row'.`j'" _tab %24.17g (el(e(`name'),`i',`j')) _n
                    }
                }
            }
        }
        file close `telemetry'
        mata: f = fopen(`"`output'/science.hex"', "w"); for (i=1; i<=cols(st_matrix("e(b)")); i++) fput(f, sprintf("b.%g %21x", i, st_matrix("e(b)")[i])); for (i=1; i<=rows(st_matrix("e(V)")); i++) for (j=1; j<=cols(st_matrix("e(V)")); j++) fput(f, sprintf("V.%g.%g %21x", i, j, st_matrix("e(V)")[i,j])); fclose(f)
        tempname science
        file open `science' using `"`output'/scalars.hex"', write text replace
        foreach scalar in N N_full N_drop rank df_m df_r df_a rss r2 r2_a F chi2 ll ll_0 deviance num_singletons {
            file write `science' "`scalar' " %21x (e(`scalar')) _n
        }
        file close `science'
        if "`output_mode'" != "none" {
            preserve
            local outputvars obs_id
            foreach pattern in bench_* saved* {
                capture unab matched : `pattern'
                if !_rc local outputvars `outputvars' `matched'
            }
            quietly keep `outputvars'
            format `outputvars' %24.17g
            quietly export delimited using `"`output'/outputs.csv"', replace datafmt
            restore
        }
    }
    quietly keep if e(sample)
    quietly keep obs_id
    quietly export delimited using `"`output'/sample.csv"', replace
}
file write `metadata' "complete" _tab "1" _n
file close `metadata'
display "FEREG_BENCH_COMPLETE rc=`rc'"
exit `rc', clear
