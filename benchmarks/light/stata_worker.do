version 19.0
clear all
set more off
set varabbrev off
set type double
set linesize 255
args request
include `"`request'"'
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
quietly use `"`warm'"', clear
capture quietly `command'
if _rc {
    display as error "LIGHT_WARMUP_FAILED rc=" _rc
    exit 459
}
ereturn clear
capture mata: mata drop BenchG
quietly use `"`data'"', clear
mata:
void light_binary(string scalar path, string rowvector names)
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
light_binary(st_local("output")+"/import.bin", tokens(st_local("inputvars")))
end
tempname mark
file open `mark' using `"`output'/INPUT.READY"', write text replace
file write `mark' "READY" _n
file close `mark'
capture confirm file `"`output'/IMPORT.CHECKED"'
while _rc {
    sleep 10
    capture confirm file `"`output'/IMPORT.CHECKED"'
}
file open `mark' using `"`output'/START"', write text replace
file write `mark' "START" _n
file close `mark'
capture confirm file `"`output'/START.CHECKED"'
while _rc {
    sleep 1
    capture confirm file `"`output'/START.CHECKED"'
}
timer clear 91
timer on 91
capture quietly `command'
local rc = _rc
local returned_n = r(N)
timer off 91
quietly timer list 91
local seconds = r(t91)
file open `mark' using `"`output'/DONE"', write text replace
file write `mark' "DONE" _n
file close `mark'

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
        mata: st_matrix("__light_b", BenchG.b)
        capture mata: st_matrix("__light_V", BenchG.Vcov)
        local have_v = (_rc == 0)
        mata: st_matrix("__light_se", BenchG.se)
        if "`arm'" == "gglm" gen byte audit_sample = !missing(audit_link)
        else gen byte audit_sample = !missing(audit_resid)
        gen double audit_xb = 0
        forvalues j=1/`k' {
            replace audit_xb = audit_xb + __light_b[1,`j']*x`j'
        }
        if `save' {
            mata: st_numscalar("__light_cons", BenchG.consest[1])
            replace audit_xb = audit_xb + __light_cons
        }
        if "`model'" == "ols" {
            mata: st_numscalar("__light_rss", BenchG.rss[1])
            file write `meta' "rss" _tab %24.17g (__light_rss) _n
        }
    }
    else {
        matrix __light_b = e(b)
        matrix __light_V = e(V)
        local have_v 1
        local scalars : e(scalars)
        foreach scalar_name of local scalars {
            file write `meta' "`scalar_name'" _tab %24.17g (e(`scalar_name')) _n
        }
        gen byte audit_sample = e(sample)
        predict double audit_xb, xb
    }
    if "`model'" == "ols" {
        gen double audit_fit = y-audit_resid if audit_sample
    }
    else if "`arm'" == "feppml" {
        gen double audit_fit = audit_mu if audit_sample
    }
    else if "`arm'" == "ppmlhdfe" {
        predict double audit_fit, mu
    }
    else {
        gen double audit_fit = exp(audit_link) if audit_sample
    }
    gen double audit_saved = .
    if `save' {
        replace audit_saved = saved1
        forvalues j=2/`dimensions' {
            replace audit_saved = audit_saved + saved`j'
        }
        if `slopes' {
            capture confirm variable saved2_Slope1
            if !_rc replace audit_saved = audit_saved + z*saved2_Slope1
            else {
                capture confirm variable saved2Slope1
                if !_rc replace audit_saved = audit_saved + z*saved2Slope1
                else error 111
            }
        }
    }
    tempname values
    file open `values' using `"`output'/coefficients.csv"', write text replace
    file write `values' "name,value,se" _n
    forvalues j=1/`k' {
        if inlist("`arm'", "greg", "gglm") local index = `j'
        else local index = colnumb(__light_b, "x`j'")
        local b = __light_b[1,`index']
        if `have_v' local se = sqrt(__light_V[`index',`index'])
        else local se = __light_se[1,`index']
        file write `values' "x`j'," %24.17g (`b') "," %24.17g (`se') _n
    }
    file close `values'
    if `have_v' {
        file open `values' using `"`output'/covariance.csv"', write text replace
        file write `values' "row,column,value" _n
        forvalues j=1/`k' {
            forvalues h=1/`k' {
                if inlist("`arm'", "greg", "gglm") {
                    local a = `j'
                    local b = `h'
                }
                else {
                    local a = colnumb(__light_b, "x`j'")
                    local b = colnumb(__light_b, "x`h'")
                }
                file write `values' "x`j',x`h'," %24.17g (__light_V[`a',`b']) _n
            }
        }
        file close `values'
    }
    foreach matrix_name in fereg_solver fereg_runtime feppml_solver feppml_runtime {
        capture matrix __light_diag = e(`matrix_name')
        if !_rc {
            file open `values' using `"`output'/`matrix_name'.csv"', write text replace
            local names : colnames __light_diag
            file write `values' "row,column,value" _n
            forvalues i=1/`=rowsof(__light_diag)' {
                forvalues j=1/`=colsof(__light_diag)' {
                    local name : word `j' of `names'
                    file write `values' "`i',`name'," %24.17g (__light_diag[`i',`j']) _n
                }
            }
            file close `values'
        }
    }
    keep if audit_sample
    mata: light_binary(st_local("output")+"/state.bin", tokens("obs_id audit_fit audit_xb audit_saved"))
}
file close `meta'
display as result "LIGHT_WORKER_COMPLETE rc=`rc'"
exit `rc'
