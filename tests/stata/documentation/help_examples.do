version 18.0
clear all
set more off

set obs 4
generate long original_id = _n
generate double original_value = _n / 10
quietly datasignature set, reset

foreach example in basic_two_way factor_if weights clusters string_fe ///
    interacted_fe absorbed_slope slope_only saved_outputs disconnected ///
    low_memory {
    capture noisily fereg_run `example' using fereg.sthlp
    assert _rc == 0
    quietly datasignature confirm
}

capture noisily fereg_run missing_example using fereg.sthlp
assert _rc == 111
quietly datasignature confirm

capture noisily fereg_run basic_two_way using missing_fereg_help.sthlp
assert _rc == 601
quietly datasignature confirm

di as result "help_examples.do: PASS"
di as result "END_FIXTURE|help_examples|rc=0"
