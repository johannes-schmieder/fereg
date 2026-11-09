version 17.0
* One frozen arm/block; scientific export is outside the command clock.
args data results arm block threads reps widths clock science
clear all
set more off
set varabbrev off
program _no_fe_clock, plugin using(`"`clock'"')
quietly use `"`data'/data.dta"', clear
assert _N==1000000
mata: ft=fopen(st_local("results"),"a"); fs=fopen(st_local("science"),"a")
foreach k of local widths {
    foreach t of local threads {
        local processors=min(`t',c(processors_lic),c(processors_mach))
        set processors `processors'
        local command "fereg y x1-x`k', threads(`t')"
        if "`arm'"=="regress" local command "regress y x1-x`k'"
        local trace : environment FEREG_SOLVER_TRACE_FILE
        if `"`trace'"'!="" {
            file open traceout using `"`trace'"', write append text
            file write traceout "FEREG_SOLVER_TRACE|phase=no_fe_benchmark|k=`k'|threads=`t'" _n
            file close traceout
        }
        forvalues r=0/`reps' {
            tempname seconds resolution
            scalar `seconds'=.
            scalar `resolution'=.
            plugin call _no_fe_clock, start
            capture quietly `command'
            local rc=_rc
            plugin call _no_fe_clock, stop `seconds' `resolution'
            if `rc' exit `rc'
            assert scalar(`seconds')>0 & scalar(`seconds')<120
            assert scalar(`resolution')>0 & scalar(`resolution')<=0.000001
            assert e(N)==_N & e(df_r)==_N-`k'-1
            quietly count if !e(sample)
            assert r(N)==0
            local native=.
            local load=.
            local total=.
            local workers=`processors'
            if "`arm'"!="regress" {
                local native=el(e(fereg_runtime),20,2)
                local load=el(e(fereg_runtime),19,2)
                local total=el(e(fereg_runtime),21,2)
                local workers=e(fereg_threads)
                assert `workers'==`t' & e(df_a)==1 & e(rank)==`k'
                assert e(N_hdfe)==0 & e(ic)==1
                if `"`trace'"'!="" {
                    * Stop code 5 is the derived residual (no residual solve).
                    local residual_stop=cond("`e(fe_residual_method)'"=="derived",5,2)
                    local residual_iterations=e(fe_residual_iterations)
                    file open traceout using `"`trace'"', write append text
                    file write traceout "FEREG_SOLVER_TRACE|phase=no_fe_result|iterations=`e(ic)'|n_hdfe=`e(N_hdfe)'|solver=`e(fe_method)'|residual_stop=`residual_stop'|residual_iterations=`residual_iterations'" _n
                    file close traceout
                }
            }
            else assert e(rank)==`k'+1
            mata: fput(ft,"`arm',`block',`k',`t',`r',`processors',`workers',"+strofreal(st_numscalar("`seconds'"),"%24.17g")+",`load',`native',`total'")
            mata: fput(fs,"`arm' `block' `k' `t' `r' "+invtokens(strofreal((st_matrix("e(b)"),vec(st_matrix("e(V)"))',st_numscalar("e(rss)"),st_numscalar("e(N)"),st_numscalar("e(N)")-st_numscalar("e(df_r)"),st_numscalar("e(df_r)")),"%24.17g")))
        }
    }
}
mata: fclose(ft); fclose(fs)
display "END_FIXTURE|no_fe_speed|rc=0"
