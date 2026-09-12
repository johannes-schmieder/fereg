version 19.0
clear all
set more off
set varabbrev off
set linesize 255
args source refs data warm output arm model k threads dimensions phase import_path
local processors = min(`threads', c(processors_lic), c(processors_mach))
set processors `processors'
sysdir set PLUS `"`refs'"'
sysdir set PERSONAL `"`output'"'
capture adopath - SITE
capture adopath - "."
adopath ++ `"`refs'"'
adopath ++ `"`source'/stata"'
adopath ++ `"`source'/dist"'
global GTOOLS_BETA "I KNOW WHAT I AM DOING"
local xvars
forvalues j = 1/`k' {
    local xvars `xvars' x`j'
}
local effects g1
if `dimensions' >= 2 local effects `effects' g2
if `dimensions' == 3 local effects `effects' g3
local solver : environment FEREG_LOCAL_SOLVER
local pool : environment FEREG_LOCAL_POOL
if "`pool'" == "" local pool 10
local command `"`arm' y `xvars', absorb(`effects')"'
if inlist("`arm'", "fereg", "feppml") local command `"`command' threads(`threads') solver(`solver') poolsize(`pool')"'
if inlist("`arm'", "greg", "gglm") local command `"`command' mata(BenchG)"'
if "`arm'" == "gglm" local command `"`command' family(poisson) robust"'
if inlist("`arm'", "feppml", "ppmlhdfe") local command `"`command' vce(robust)"'

if "`phase'" == "warm" {
    quietly use `"`warm'"', clear
    capture quietly `command'
    local warm_rc = _rc
    if `warm_rc' {
        display as error "SYSTEMATIC_WARMUP_FAILED rc=`warm_rc'"
        exit `warm_rc'
    }
    ereturn clear
    capture mata: mata drop BenchG
}
quietly use `"`data'"', clear
local inputvars obs_id `effects' `xvars' y
mata:
void __sb_export_input(string scalar path, string rowvector names)
{
    real scalar fh, j, first, last, n
    real colvector control
    fh = fopen(path, "w")
    control = bufio()
    n = st_nobs()
    for (j=1; j<=cols(names); j++) {
        for (first=1; first<=n; first=first+8192) {
            last = min((n, first+8191))
            fbufput(control, fh, "%8z", st_data(first::last, names[j]))
        }
    }
    fclose(fh)
}
__sb_export_input(st_local("import_path"), tokens(st_local("inputvars")))
end
tempname mark
file open `mark' using `"`output'/INPUT.READY"', write text replace
file write `mark' "READY" _n
file close `mark'
capture confirm file `"`output'/IMPORT.CHECKED"'
while _rc {
    sleep 50
    capture confirm file `"`output'/IMPORT.CHECKED"'
}
file open `mark' using `"`output'/READY"', write text replace
file write `mark' "READY" _n
file close `mark'
sleep 100
file open `mark' using `"`output'/START"', write text replace
file write `mark' "START" _n
file close `mark'
capture confirm file `"`output'/START.CHECKED"'
while _rc {
    sleep 10
    capture confirm file `"`output'/START.CHECKED"'
}
local trace : environment FEREG_LOCAL_TRACE
if "`trace'" == "1" profiler on
timer clear 91
timer on 91
capture quietly `command'
local rc = _rc
local returned_n = r(N)
if "`trace'" == "1" profiler off
timer off 91
quietly timer list 91
local seconds = r(t91)
file open `mark' using `"`output'/DONE"', write text replace
file write `mark' "DONE" _n
file close `mark'

if "`trace'" == "1" profiler report

tempname meta
file open `meta' using `"`output'/metadata.tsv"', write text replace
file write `meta' "rc" _tab "`rc'" _n "seconds" _tab %24.17g (`seconds') _n
file write `meta' "stata_processors" _tab "`processors'" _n
file write `meta' "processors_lic" _tab "`c(processors_lic)'" _n
file write `meta' "processors_mach" _tab "`c(processors_mach)'" _n
file write `meta' "stata_version" _tab "`c(stata_version)'" _n
file write `meta' "command" _tab `"`command'"' _n
if !`rc' {
    if inlist("`arm'", "greg", "gglm") {
        file write `meta' "N" _tab %24.17g (`returned_n') _n
        mata: st_matrix("__sb_b", BenchG.b)
        capture mata: st_matrix("__sb_V", BenchG.Vcov)
        local vcov_available = (_rc == 0)
        mata: st_matrix("__sb_se", BenchG.se)
        file write `meta' "sample_kind" _tab "count_only" _n
    }
    else {
        matrix __sb_b = e(b)
        matrix __sb_V = e(V)
        local vcov_available 1
        local scalars : e(scalars)
        foreach scalar_name of local scalars {
            file write `meta' "`scalar_name'" _tab %24.17g (e(`scalar_name')) _n
        }
        file write `meta' "sample_kind" _tab "observation_ids" _n
        preserve
        quietly keep if e(sample)
        keep obs_id
        quietly export delimited using `"`output'/sample.csv"', replace
        restore
    }
    tempname values
    file open `values' using `"`output'/coefficients.csv"', write text replace
    file write `values' "name,value,se" _n
    forvalues j = 1/`k' {
        local b = __sb_b[1,`j']
        if `vcov_available' local se = sqrt(__sb_V[`j',`j'])
        else local se = __sb_se[1,`j']
        file write `values' "x`j'," %24.17g (`b') "," %24.17g (`se') _n
    }
    file close `values'
    if `vcov_available' {
        file open `values' using `"`output'/covariance.csv"', write text replace
        file write `values' "row,column,value" _n
        forvalues j = 1/`k' {
            forvalues h = 1/`k' {
                file write `values' "x`j',x`h'," %24.17g (__sb_V[`j',`h']) _n
            }
        }
        file close `values'
    }
    foreach matrix_name in b V fereg_solver fereg_runtime fereg_certificate fereg_fit fereg_status fereg_dof {
        capture matrix __sb_diag = e(`matrix_name')
        if !_rc {
            file open `values' using `"`output'/`matrix_name'.csv"', write text replace
            local nr = rowsof(__sb_diag)
            local nc = colsof(__sb_diag)
            file write `values' "row,column,value" _n
            forvalues j = 1/`nr' {
                forvalues h = 1/`nc' {
                    file write `values' "`j',`h'," %24.17g (__sb_diag[`j',`h']) _n
                }
            }
            file close `values'
        }
    }
}
file close `meta'
display as result "SYSTEMATIC_WORKER_COMPLETE rc=`rc'"
exit 0
