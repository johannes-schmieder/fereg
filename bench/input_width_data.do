version 17.0
args out
clear all
set more off
set seed 739391
set obs 100003
forvalues j=1/129 {
    if mod(`j',4) == 0 generate float x`j' = rnormal()
    else generate double x`j' = rnormal()
}
generate long g = mod(_n*31,2003) + 1
generate double y = x1 + .5*x2 - .25*x3 + sin(g/11) + rnormal()
save `"`out'/inputs_width.dta"', replace
display "END_FIXTURE|input_width_data|rc=0"
