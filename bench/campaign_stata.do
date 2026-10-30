version 19.0
clear all
set more off
set varabbrev off
set linesize 255
args configuration
include `"`configuration'"'
program _campaign_wait
    args path
    local waited = 0
    capture confirm file `"`path'"'
    while _rc {
        sleep 1
        local ++waited
        if `waited' > 120000 {
            display as error "campaign handshake timed out: `path'"
            exit 9
        }
        capture confirm file `"`path'"'
    }
end
local processors = min(`threads', 4, c(processors_lic), c(processors_mach))
set processors `processors'
sysdir set PLUS `"`refs'"'
sysdir set PERSONAL `"`output'"'
capture adopath - SITE
capture adopath - "."
adopath ++ `"`refs'"'
adopath ++ `"`source'/stata"'
adopath ++ `"`source'/dist"'
if "`arm'" == "fereg" {
    quietly findfile fereg_linux.plugin
    assert `"`r(fn)'"' == `"`source'/dist/fereg_linux.plugin"'
}
local xvars
forvalues j=1/`k' {
    local xvars `xvars' x`j'
}
local absorb
forvalues j=1/`effects' {
    local term g`j'
    if `save_fe' local term saved`j'=g`j'
    local absorb `absorb' `term'
}
local inference vce(`vce')
if "`vce'" == "iid" local inference vce(unadjusted)
if "`vce'" == "iid" & "`arm'" == "areg" local inference vce(ols)
if "`vce'" == "cluster" local inference vce(cluster cluster)
local command `arm'
if "`arm'" == "fereg" & "`model'" == "ppml" local command feppml
* Stata 19 areg absorbs several categorical effects but cannot save them.
if "`arm'" == "areg" & ("`model'" != "ols" | `save_fe') exit 198
local options
if "`arm'" == "fereg" local options threads(`threads')
if "`tolerance'" != "" local options `options' tolerance(`tolerance')
* Multi-effect areg uses the exact-collinearity DoF adjustment and the
* tolerance of the August 2026 comparison, without the absorbed-effects F test.
local timed_options `options'
if "`arm'" == "areg" & `effects' > 1 local timed_options `options' dfabsorb noabsorbtest tolerance(1e-8) iterate(16000)
* Initialization uses a separate small input and is excluded from timing.
quietly use `"`warm'"', clear
capture noisily quietly `command' y `xvars', absorb(g1) `inference' `options'
local init_rc = _rc
if `init_rc' exit `init_rc', clear
quietly use `"`dataset'"', clear
program _campaign_clock, plugin using(`"`clock_plugin'"')
tempname command_seconds resolution
scalar `command_seconds' = .
scalar `resolution' = .
display "CAMPAIGN_READY"
file open ready using `"`output'/READY"', write text replace
file write ready "ready" _n
file close ready
* The harness resets the kernel memory peak before GO and reads it before ACK.
_campaign_wait `"`output'/GO"'
plugin call _campaign_clock, start
capture noisily quietly `command' y `xvars', absorb(`absorb') `inference' `timed_options'
local rc = _rc
plugin call _campaign_clock, stop `command_seconds' `resolution'
file open done using `"`output'/DONE"', write text replace
file write done "done" _n
file close done
_campaign_wait `"`output'/ACK"'
tempname meta
file open `meta' using `"`output'/metadata.tsv"', write text replace
file write `meta' "rc" _tab "`rc'" _n
file write `meta' "seconds" _tab %24.17g (scalar(`command_seconds')) _n
file write `meta' "clock_resolution" _tab %24.17g (scalar(`resolution')) _n
file write `meta' "processors" _tab "`processors'" _n
file write `meta' "processors_licensed" _tab %24.17g (c(processors_lic)) _n
if !`rc' {
    if "`arm'" == "fereg" {
        if "`model'" == "ols" {
            local row = 18
            foreach phase in preflight input native total {
                file write `meta' "`phase'_seconds" _tab %24.17g (el(e(fereg_runtime),`row',2)) _n
                local ++row
            }
        }
        else {
            file write `meta' "total_seconds" _tab %24.17g (el(e(feppml_runtime),1,1)) _n
            local phases : rownames e(feppml_runtime)
            forvalues row=28/40 {
                local phase : word `row' of `phases'
                file write `meta' "`phase'" _tab %24.17g (el(e(feppml_runtime),`row',1)) _n
            }
        }
    }
    foreach scalar in N rank df_a df_m df_r df_resid N_clust num_singletons separated converged rss ll deviance fereg_threads feppml_threads {
        capture local value = e(`scalar')
        if !_rc file write `meta' "`scalar'" _tab %24.17g (`value') _n
    }
    tempname coefficients covariance
    matrix `coefficients' = e(b)
    matrix `covariance' = e(V)
    _ms_omit_info e(b)
    tempname omitted
    matrix `omitted' = r(omit)
    local slope_omissions = 0
    local names : colnames e(b)
    forvalues j=1/`=colsof(e(b))' {
        local name : word `j' of `names'
        if !inlist("`name'", "_cons", "o._cons") local slope_omissions = `slope_omissions' + el(`omitted',1,`j')
    }
    file write `meta' "slope_omissions" _tab "`slope_omissions'" _n
    file write `meta' "finite_slope_rank" _tab "`=`k'-`slope_omissions''" _n
    file write `meta' "covariance_policy" _tab "stock-`model'-`vce'" _n
    mata: b=st_matrix("`coefficients'"); V=st_matrix("`covariance'"); stripe=st_matrixcolstripe("`coefficients'"); f=fopen(`"`output'/coefficients.csv"', "w"); fput(f,"name,value"); for(i=1;i<=cols(b);i++) fput(f,sprintf("%s,%24.17g",stripe[i,2],b[i])); fclose(f); f=fopen(`"`output'/covariance.csv"',"w"); fput(f,"row,column,value"); for(i=1;i<=rows(V);i++) for(j=1;j<=cols(V);j++) fput(f,sprintf("%s,%s,%24.17g",stripe[i,2],stripe[j,2],V[i,j])); fclose(f)
    quietly generate byte retained = e(sample)
    if `save_fe' {
        quietly generate double fitted = 0 if retained
        forvalues j=1/`k' {
            quietly replace fitted = fitted + _b[x`j']*x`j' if retained
        }
        capture local constant = _b[_cons]
        if !_rc quietly replace fitted = fitted + `constant' if retained
        forvalues j=1/`effects' {
            quietly replace fitted = fitted + saved`j' if retained
        }
        if "`model'" == "ppml" quietly replace fitted = exp(fitted) if retained
        local outvars obs_id retained fitted saved*
    }
    else local outvars obs_id retained
    preserve
    quietly keep `outvars'
    format obs_id %12.0f
    capture format fitted saved* %24.17g
    quietly export delimited using `"`output'/rows.csv"', replace datafmt
    restore
}
file write `meta' "complete" _tab "1" _n
file close `meta'
display "CAMPAIGN_COMPLETE rc=`rc'"
exit `rc', clear
