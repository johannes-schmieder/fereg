version 18.0
clear all
set more off

global FEREG_PPML_TEST_COMMAND "_fereg_ppml"
findfile feppml_separation_outputs.do
do `"`r(fn)'"'
capture macro drop FEREG_PPML_TEST_COMMAND

display as result "END_FIXTURE|estimate_ppml_hidden_separation_outputs|rc=0"
