version 19.0
clear all
set more off
args clock_plugin
program __twofe_clock, plugin using(`"`clock_plugin'"')
tempname elapsed
scalar `elapsed' = .
capture plugin call __twofe_clock, end `elapsed'
assert _rc == 198
capture plugin call __twofe_clock, bad
assert _rc == 198
plugin call __twofe_clock, begin
capture plugin call __twofe_clock, begin
assert _rc == 198
plugin call __twofe_clock, end `elapsed'
assert scalar(`elapsed') >= 0 & scalar(`elapsed') < 1
capture plugin call __twofe_clock, end `elapsed'
assert _rc == 198
forvalues j = 1/3 {
    plugin call __twofe_clock, begin
    sleep 50
    plugin call __twofe_clock, end `elapsed'
    assert scalar(`elapsed') >= .045 & scalar(`elapsed') < .5
    display "CLOCK_SLEEP_SECONDS|" %24.17g scalar(`elapsed')
}
display "HIGHRES_CLOCK_FIXTURE_PASS"
exit 0
