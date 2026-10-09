version 18.0
clear all
set more off

global FEREG_PPML_TEST_COMMAND "_fereg_ppml"
global FEREG_PPML_HELP_SOURCE "feppml.sthlp"
findfile feppml_help_examples.do
do `"`r(fn)'"'
capture macro drop FEREG_PPML_HELP_SOURCE
capture macro drop FEREG_PPML_TEST_COMMAND

display as result "END_FIXTURE|estimate_ppml_hidden_help_examples|rc=0"
