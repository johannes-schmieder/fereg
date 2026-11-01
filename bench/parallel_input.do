version 17.0
* One arm of one block of the parallel numeric input comparison.
* usage: do parallel_input.do <data dir> <results csv> <arm> <block> "<threads>" <reps>
* Cells (frozen): see bench/parallel_input.py. Every timed call is posted.
args data results arm block threads reps
clear all
set more off
local c1 "fereg y x1 x2, absorb(g)"
local c2 "fereg y x1-x64,"
local c3 "fereg y x1-x32, absorb(g)"
local c4 "fereg y x1-x16 if keep, absorb(g)"
local c5 "fereg y x1-x8, absorb(sg)"
local c6 "fereg y x1-x4, absorb(g)"
local c7 "feppml py x1-x4 if keep, absorb(g) exposure(e)"
local c8 "fereg y x1-x32, absorb(g) memory(low)"
* Restricted input coverage: 1% scattered, short contiguous original range,
* weights/multiple FE, string FE, and PPML exposure.
local c9 "fereg y x1 x2 if mod(row,100)==0, absorb(g)"
local c10 "fereg y x1-x64 if mod(row,100)==0,"
local c11 "fereg y x1-x4 in 4995001/5005000,"
local c12 "fereg y x1-x8 if mod(row,100)==0 [aw=w], absorb(g g2)"
local c13 "fereg y x1-x8 if mod(row,100)==0, absorb(sg)"
local c14 "feppml py x1-x4 if mod(row,100)==0, absorb(g) exposure(e)"
tempname handle
file open `handle' using `"`results'"', write append text
foreach cell in c1 c2 c3 c4 c5 c6 c7 c8 c9 c10 c11 c12 c13 c14 {
    local file = cond(inlist("`cell'", "c6", "c11"), "inputs_10m", "inputs_1m")
    quietly use `"`data'/`file'.dta"', clear
    local command ``cell''
    local ppml = substr(`"`command'"', 1, 6) == "feppml"
    foreach t of local threads {
        local trace : environment FEREG_SOLVER_TRACE_FILE
        if `"`trace'"' != "" {
            tempname trace_handle
            file open `trace_handle' using `"`trace'"', write append text
            file write `trace_handle' "FEREG_SOLVER_TRACE|phase=input_benchmark|cell=`cell'|threads=`t'" _n
            file close `trace_handle'
        }
        * One untimed warmup per cell and worker count.
        quietly `command' threads(`t')
        forvalues r = 1/`reps' {
            timer clear 1
            timer on 1
            quietly `command' threads(`t')
            timer off 1
            quietly timer list 1
            local wall = r(t1)
            if `ppml' {
                local load = .
                local native = el(e(feppml_runtime), 1, 1)
                local alloc = .
            }
            else {
                local load = el(e(fereg_runtime), 19, 2)
                local native = el(e(fereg_runtime), 21, 2)
                local alloc = el(e(fereg_runtime), 12, 2)
            }
            file write `handle' "`arm',`block',`cell',`t',`r',`=_N',`e(N)',`wall',`load',`native',`alloc'" _n
        }
    }
}
file close `handle'
display as result "END_FIXTURE|parallel_input|rc=0"
