version 19.0
clear all
set more off
args refs
set processors 4
sysdir set PLUS `"`refs'"'
sysdir set PERSONAL `"`refs'"'
capture adopath - SITE
capture adopath - "."
adopath ++ `"`refs'"'
capture noisily ftools, compile
local rc = _rc
if !`rc' {
    capture noisily {
        findfile lftools.mlib
        assert `"`r(fn)'"' == `"`refs'/l/lftools.mlib"'
    }
    local rc = _rc
}
display "CAMPAIGN_REFERENCES_COMPLETE rc=`rc'"
exit `rc', clear
