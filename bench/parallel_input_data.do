version 17.0
* Frozen inputs for the parallel numeric input comparison (bench/parallel_input.py).
* usage: do parallel_input_data.do <output directory> [1m]
* The optional 1m argument writes only the 1M-row file (paired host checks).
args out only
clear all
set more off
set seed 739391
set obs 1000000
generate long row = _n
forvalues j = 1/64 {
    if mod(`j', 4) == 0 generate float x`j' = sin(_n * `j' / 97)
    else generate double x`j' = rnormal()
}
generate long g = mod(_n * 31, 5003) + 1
generate str8 sg = "s" + string(mod(_n * 17, 1009))
generate byte keep = mod(_n * 7, 10) < 5
generate double y = x1 + .5 * x2 - .25 * x3 + .1 * x4 + sin(g / 11) + rnormal()
generate double e = exp(.01 * mod(_n, 7))
generate double py = exp(.2 * x1 + .1 * x2 + sin(g / 11) / 4) * e
quietly replace x5 = . if mod(_n, 997) == 0
generate int g2 = mod(_n, 97)
generate double w = 1 + mod(_n, 5)
save `"`out'/inputs_1m.dta"', replace
if `"`only'"' == "1m" {
    display as result "END_FIXTURE|parallel_input_data|rc=0"
    exit
}
clear
set obs 10000000
generate double x1 = rnormal()
generate double x2 = rnormal()
generate float x3 = sin(_n / 89)
generate double x4 = rnormal()
generate long g = mod(_n * 31, 50021) + 1
generate double y = x1 + .5 * x2 - .25 * x3 + .1 * x4 + sin(g / 11) + rnormal()
save `"`out'/inputs_10m.dta"', replace
display as result "END_FIXTURE|parallel_input_data|rc=0"
