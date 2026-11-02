version 17.0
* Source columns include y and the FE source, not the reported constant.
args data results arm block threads reps extra
clear all
set more off
quietly use `"`data'/inputs_width.dta"', clear
local science : environment FEREG_WIDTH_SCIENCE
local trace : environment FEREG_SOLVER_TRACE_FILE
tempname handle B V B0 V0
file open `handle' using `"`results'"', write append text
foreach width in 63 64 65 66 130 {
    foreach effect in none fe {
        local k = `width' - 1 - ("`effect'"=="fe")
        local absorb = cond("`effect'"=="fe", "absorb(g)", "")
        if `"`science'"' != "" {
            if "`effect'" == "fe" quietly areg y x1-x`k', absorb(g)
            else quietly regress y x1-x`k'
            matrix `B0' = e(b)
            matrix `V0' = e(V)
        }
        foreach mode in parallel serial {
            local cell "w`width'-`effect'-`mode'"
            local command = cond("`mode'"=="serial", "_fereg_estimate_ols_test", "fereg")
            local injection = cond("`mode'"=="serial", "inject(serial_input)", "")
            foreach t of local threads {
                if `"`trace'"' != "" {
                    file open widetrace using `"`trace'"', write append text
                    file write widetrace "FEREG_SOLVER_TRACE|phase=input_benchmark|cell=`cell'|threads=`t'" _n
                    file close widetrace
                }
                forvalues r = 0/`reps' {
                    timer clear 1
                    timer on 1
                    quietly `command' y x1-x`k', `absorb' `injection' threads(`t')
                    timer off 1
                    quietly timer list 1
                    local wall = r(t1)
                    local load = el(e(fereg_runtime),19,2)
                    local native = el(e(fereg_runtime),21,2)
                    local alloc = el(e(fereg_runtime),12,2)
                    if `r' > 0 file write `handle' "`arm',`block',`cell',`t',`r',`=_N',`e(N)',`wall',`load',`native',`alloc'" _n
                    if `"`science'"' != "" {
                        matrix `B' = e(b)
                        matrix `V' = e(V)
                        mata: assert(max(abs(st_matrix("`B'")-st_matrix("`B0'"))) < 1e-8)
                        mata: assert(max(abs(st_matrix("`V'")-st_matrix("`V0'"))) < 1e-8)
                        tempvar sample
                        quietly generate byte `sample' = e(sample)
                        quietly _datasignature `sample', nonames
                        local sample_signature `"`r(datasignature)'"'
                        quietly drop `sample'
                        mata: fh=fopen(st_local("science"),"a"); fput(fh,st_local("cell")+"|"+invtokens(strofreal(vec(st_matrix("`B'"))',"%21x"))+"|"+invtokens(strofreal(vec(st_matrix("`V'"))',"%21x"))+"|"+strofreal(st_numscalar("e(N)"),"%21x")+"|"+st_local("sample_signature")); fclose(fh)
                    }
                }
            }
        }
    }
}
file close `handle'
if `"`science'"' != "" do `"`extra'"' `"`science'"' `"`threads'"' `"`trace'"'
display "END_FIXTURE|input_width|rc=0"
