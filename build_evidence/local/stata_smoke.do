version 18.0
clear all
set more off
set trace on
set tracedepth 2
adopath ++ "/Users/johannes/Git/StFixest/stata"
adopath ++ "/Users/johannes/Git/StFixest/dist"
do "/Users/johannes/Git/StFixest/tests/stata/installation/phase01_smoke.do"
do "/Users/johannes/Git/StFixest/tests/stata/installation/reload_loop.do"
exit
