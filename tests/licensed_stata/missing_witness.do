version 17.0
clear all
set more off
* usage: do missing_witness.do <compiled missing_witness plugin>
* The scalar reader's SF_is_missing and the batch/parallel readers'
* `value >= SV_missval` must agree on every value the SDK can return, and the
* checked and unchecked accessors must return the same bits.
args plugin
confirm file `"`plugin'"'
program missing_witness, plugin using(`"`plugin'"')
set obs 40
generate double d = .
generate float f = .
plugin call missing_witness d f, store
local codes ". .a .b .c .d .e .f .g .h .i .j .k .l .m .n .o .p .q .r .s .t .u .v .w .x .y .z"
forvalues j = 1/27 {
    local code : word `j' of `codes'
    quietly replace d = `code' in `=12 + `j''
    quietly replace f = `code' in `=12 + `j''
}
quietly replace d = 1e300 in 40
quietly replace f = -1e30 in 40
tempfile witness
log using `witness', text replace name(witness)
plugin call missing_witness d f, check
log close witness
tempname handle
file open `handle' using `witness', read text
local rows 0
file read `handle' line
while r(eof) == 0 {
    if substr(`"`line'"', 1, 2) == "W|" {
        local ++rows
        * W|obs|var|unchecked bits|checked bits|ismissing|value >= missval
        tokenize `"`line'"', parse("|")
        local unchecked "`7'"
        local checked "`9'"
        local ismissing "`11'"
        local ge "`13'"
        display as text "obs `3' var `5': `unchecked' ismissing=`ismissing' ge=`ge'"
        assert "`unchecked'" == "`checked'"
        assert "`ismissing'" == "`ge'"
    }
    file read `handle' line
}
file close `handle'
assert `rows' == 80
display as result "END_FIXTURE|missing_witness|rc=0"
