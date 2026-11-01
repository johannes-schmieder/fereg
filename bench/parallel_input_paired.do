version 18.0
* Same-process paired timing of the serial and parallel numeric readers of one
* plugin, for a host whose numbers cannot leave the machine (the Windows
* controller returns only a sanitized receipt). The hidden adapters run each
* frozen 1M-row cell of bench/parallel_input.do (the 10M-row and memory(low)
* cells are omitted for the time ceiling) with inject(serial_input), which is
* the serial reader every unqualified host uses, and without it. Arms alternate
* per repetition after one warmup each. Frozen rule (bench/parallel_input.py):
* a cell regresses when the parallel median wall time exceeds the serial median
* by more than 5% and more than 1 ms; any regression exits 9.
* usage: do parallel_input_paired.do <scratch dir> <threads> <reps>
args scratch threads reps
confirm integer number `threads'
confirm integer number `reps'
assert `threads' >= 2 & `reps' >= 3
quietly do "bench/parallel_input_data.do" `"`scratch'"' 1m
local c1 "_fereg_estimate_ols_test y x1 x2, absorb(g)"
local c2 "_fereg_estimate_ols_test y x1-x64,"
local c3 "_fereg_estimate_ols_test y x1-x32, absorb(g)"
local c4 "_fereg_estimate_ols_test y x1-x16 if keep, absorb(g)"
local c5 "_fereg_estimate_ols_test y x1-x8, absorb(sg)"
local c7 "_fereg_estimate_ppml py x1-x4 if keep, absorb(g) exposure(e)"
mata:
real scalar _paired_median(real colvector x)
{
    real colvector s
    real scalar n
    s = sort(select(x, x :< .), 1)
    n = rows(s)
    if (n == 0) return(.)
    return(mod(n, 2) ? s[(n + 1) / 2] : (s[n / 2] + s[n / 2 + 1]) / 2)
}
end
quietly use `"`scratch'/inputs_1m.dta"', clear
local regressions 0
foreach cell in c1 c2 c3 c4 c5 c7 {
    local command ``cell''
    local ols = "`cell'" != "c7"
    quietly `command' threads(`threads') inject(serial_input)
    quietly `command' threads(`threads')
    tempname wall
    matrix `wall' = J(`reps', 4, .)
    forvalues r = 1/`reps' {
        foreach position in 1 2 {
            * Odd repetitions run serial first, even ones parallel first.
            local arm = cond(mod(`r', 2) == (`position' == 1), 1, 2)
            local injection = cond(`arm' == 1, "inject(serial_input)", "")
            timer clear 1
            timer on 1
            quietly `command' threads(`threads') `injection'
            timer off 1
            quietly timer list 1
            matrix `wall'[`r', `arm'] = r(t1)
            local load_column = `arm' + 2
            if `ols' matrix `wall'[`r', `load_column'] = el(e(fereg_runtime), 19, 2)
        }
    }
    mata: st_local("serial", strofreal(_paired_median(st_matrix("`wall'")[., 1])))
    mata: st_local("parallel", strofreal(_paired_median(st_matrix("`wall'")[., 2])))
    mata: st_local("load_serial", strofreal(_paired_median(st_matrix("`wall'")[., 3])))
    mata: st_local("load_parallel", strofreal(_paired_median(st_matrix("`wall'")[., 4])))
    local flag = cond(`parallel' > 1.05 * `serial' & `parallel' - `serial' > .001, "REGRESSION", "ok")
    if "`flag'" == "REGRESSION" local ++regressions
    display as result "PAIRED|`cell'|threads=`threads'|wall_serial=" %9.4f `serial' ///
        "|wall_parallel=" %9.4f `parallel' "|ratio=" %6.3f `parallel' / `serial' ///
        "|load_serial=" %9.4f `load_serial' "|load_parallel=" %9.4f `load_parallel' "|`flag'"
}
if `regressions' exit 9
display as result "END_FIXTURE|parallel_input_paired|rc=0"
