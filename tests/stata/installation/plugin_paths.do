version 15.0
clear all
set more off

quietly _fereg_load, action(selftest) threads(1)
local native `"`r(plugin_name)'"'
local source `"`r(plugin_file)'"'
local original_pwd `"`c(pwd)'"'
tempname folder
local relative "fereg plugin path `folder'"
mkdir `"`relative'"'
local absolute `"`original_pwd'/`relative'"'
copy `"`source'"' `"`absolute'/`native'"'

* Keep all test files in the runner's disposable working directory. On Unix,
* reach its absolute path through ~ even when that directory is outside home.
local tilde ""
if c(os) != "Windows" {
    quietly cd "~"
    local parent `"`c(pwd)'"'
    local prefix "~"
    while `"`parent'"' != "/" & `"`parent'"' != "" {
        local prefix "`prefix'/.."
        mata: st_local("parent", pathgetparent(st_local("parent")))
    }
    quietly cd `"`original_pwd'"'
    local tilde `"`prefix'`absolute'"'
}

set obs 120
generate byte a = mod(_n - 1, 4)
generate byte b = mod(floor((_n - 1)/4), 5)
generate double x = sin(_n)
generate double y = 2 + 3*x + a - b/3 + cos(_n)
generate double count = 1 + mod(_n, 7)
quietly regress y x i.a i.b
scalar ols_slope = _b[x]
quietly poisson count x i.a i.b
scalar ppml_slope = _b[x]
mata: dir = "user dir"; base = "user base"

foreach path in `"`tilde'"' `"`absolute'"' `"./`relative'"' `"`absolute'/../`relative'"' {
    if `"`path'"' == "" continue
    adopath ++ `"`path'"'
    quietly findfile `native'
    local found `"`r(fn)'"'
    assert strpos(`"`found'"', `"`relative'"') > 0
    if substr(`"`path'"', 1, 1) == "~" {
        assert substr(`"`found'"', 1, 1) == "~"
    }
    * Drop the ado and its private programs so a cached plugin cannot hide a
    * path failure. Each iteration must register the plugin again.
    capture program drop _fereg_load
    quietly _fereg_load, action(selftest) threads(1)
    local loaded = subinstr(`"`r(plugin_file)'"', "\", "/", .)
    local expected = subinstr(`"`absolute'/`native'"', "\", "/", .)
    assert `"`loaded'"' == `"`expected'"'
    quietly fereg y x, absorb(a b) threads(1)
    assert reldif(_b[x], scalar(ols_slope)) < 1e-10
    quietly feppml count x, absorb(a b) threads(1)
    assert reldif(_b[x], scalar(ppml_slope)) < 1e-7
    assert `"`c(pwd)'"' == `"`original_pwd'"'
    mata: assert(dir == "user dir" & base == "user base")
    adopath - `"`path'"'
}

capture program drop _fereg_load
erase `"`absolute'/`native'"'
rmdir `"`absolute'"'
di as result "END_FIXTURE|plugin_paths|rc=0"
