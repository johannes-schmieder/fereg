version 17.0
* fixture-requires: input-trace
clear all
set more off

* Parallel numeric input is a transport change only. Every case runs with the
* serial reader (threads(1), and inject(serial_input) at the parallel worker
* count) and with the parallel window; the plugin's input digest (solver
* trace) must agree across all of them, and equal worker counts must give
* bit-identical results. The runner checks the digests in the trace file.
local trace : environment FEREG_SOLVER_TRACE_FILE
if `"`trace'"' == "" {
    di as error "set FEREG_SOLVER_TRACE=1 and FEREG_SOLVER_TRACE_FILE"
    exit 198
}
local workers = min(4, c(processors_mach), c(processors_lic))
* Stress runs may raise the parallel worker count (fixture input only).
local stress : environment FEREG_ORACLE_WORKERS
if `"`stress'"' != "" local workers = `stress'
assert `workers' >= 2

program define _parallel_case
    args trace label
    tempname handle
    file open `handle' using `"`trace'"', write append text
    file write `handle' "FEREG_SOLVER_TRACE|phase=case|label=`label'" _n
    file close `handle'
end

set seed 20261101
set obs 70003
generate long row = _n
generate byte b = mod(_n, 120) - 60
generate int i = mod(_n * 7, 30000) - 15000
generate long l = mod(_n * 13, 2000000) - 1000000
generate float f = runiform()
generate double d = rnormal()
generate double y = d + .5 * f + .001 * b + 1e-6 * l + 1e-5 * i + rnormal()
generate double py = exp(.2 * d + .1 * f + .001 * b) * (1 + mod(_n, 3))
generate int g1 = mod(_n, 503)
generate int g2 = mod(_n * 7, 97)
generate str6 sg = "s" + string(mod(_n * 11, 211))
generate double w = 1 + mod(_n, 5)
generate double off = .01 * mod(_n, 7)
generate double expo = exp(off)
* Every numeric missing code in every storage type, spread across blocks.
local codes ". .a .b .c .d .e .f .g .h .i .j .k .l .m .n .o .p .q .r .s .t .u .v .w .x .y .z"
forvalues j = 1/27 {
    local code : word `j' of `codes'
    foreach v in b i l f d {
        quietly replace `v' = `code' in `=2567 * `j' + strlen("`v'")'
    }
    quietly replace off = `code' in `=2591 * `j''
    quietly replace expo = `code' in `=2591 * `j''
}
generate byte keep_scattered = mod(_n, 3) == 0 | mod(_n, 7) == 5
generate byte keep_tiny = mod(_n, 5000) == 17

local model1_vars "y b i l f d"
local model1_opts "absorb(g1)"
local model2_vars "y b i l f d"
local model2_opts ""
local model3_vars "y i.g2 b f d"
local model3_opts "absorb(g1)"
local model4_vars "y b i l f d"
local model4_opts "absorb(g1 g2) vce(cluster g2)"
* The transaction adapter takes clustering as vce(cluster) cluster(varlist).
local model4_hidden "absorb(g1 g2) vce(cluster) cluster(g2)"
local model5_vars "y b f d"
local model5_opts "absorb(sg g1)"
local model6_vars "y b d"
local model6_opts "absorb(g2##c.f)"
local sample1 ""
local sample2 "if keep_scattered"
local sample3 "in 1001/65000"
local sample4 "if keep_scattered in 2000/60000"
local weight1 ""
local weight2 "[aw=w]"
local weight3 "[fw=w]"
local weight4 "[pw=w]"
local wopt1 ""
local wopt2 "aweight(w)"
local wopt3 "fweight(w)"
local wopt4 "pweight(w)"

forvalues m = 1/6 {
    local vars `model`m'_vars'
    local options `model`m'_opts'
    local hidden `model`m'_hidden'
    if `"`hidden'"' == "" local hidden `options'
    forvalues s = 1/4 {
        local sample `sample`s''
        forvalues k = 1/4 {
            if `m' > 2 & `k' > 1 continue
            local weight `weight`k''
            local wopt `wopt`k''
            local label ols_m`m'_s`s'_w`k'
            _parallel_case `"`trace'"' `label'|serial
            quietly fereg `vars' `sample' `weight', `options' threads(1)
            matrix serial_b = e(b)
            matrix serial_V = e(V)
            generate byte serial_sample = e(sample)
            _parallel_case `"`trace'"' `label'|forced
            quietly _fereg_estimate_ols_test `vars' `sample', `wopt' ///
                `hidden' threads(`workers') inject(serial_input)
            matrix forced_b = e(b)
            matrix forced_V = e(V)
            _parallel_case `"`trace'"' `label'|parallel
            quietly _fereg_estimate_ols_test `vars' `sample', `wopt' ///
                `hidden' threads(`workers')
            assert mreldif(e(b), forced_b) == 0
            assert mreldif(e(V), forced_V) == 0
            assert mreldif(e(b), serial_b) < 1e-9
            assert mreldif(e(V), serial_V) < 1e-8
            assert e(sample) == serial_sample
            drop serial_sample
            assert row == _n
        }
    }
}

* A tiny sample and an empty sample keep the original behavior.
_parallel_case `"`trace'"' ols_tiny|serial
quietly fereg y b f d if keep_tiny, threads(1)
matrix tiny_b = e(b)
_parallel_case `"`trace'"' ols_tiny|parallel
quietly fereg y b f d if keep_tiny, threads(`workers')
assert mreldif(e(b), tiny_b) == 0
_parallel_case `"`trace'"' ols_empty|serial
capture fereg y b f d if row < 0, threads(1)
local empty_rc = _rc
assert `empty_rc' != 0
_parallel_case `"`trace'"' ols_empty|parallel
capture fereg y b f d if row < 0, threads(`workers')
assert _rc == `empty_rc'

* PPML with offset/exposure and restricted samples.
forvalues p = 1/4 {
    local sample `sample`p''
    foreach adjustment in offset exposure {
        local variable = cond("`adjustment'" == "offset", "off", "expo")
        local label ppml_s`p'_`adjustment'
        _parallel_case `"`trace'"' `label'|serial
        quietly feppml py b f d `sample', absorb(g1) `adjustment'(`variable') threads(1)
        matrix serial_b = e(b)
        _parallel_case `"`trace'"' `label'|forced
        quietly _fereg_estimate_ppml py b f d `sample', absorb(g1) ///
            `adjustment'(`variable') threads(`workers') inject(serial_input)
        matrix forced_b = e(b)
        matrix forced_V = e(V)
        _parallel_case `"`trace'"' `label'|parallel
        quietly _fereg_estimate_ppml py b f d `sample', absorb(g1) ///
            `adjustment'(`variable') threads(`workers')
        assert mreldif(e(b), forced_b) == 0
        assert mreldif(e(V), forced_V) == 0
        assert mreldif(e(b), serial_b) < 1e-8
    }
}

* Short original-row envelopes, including Stata's negative and L bounds.
local range1 "in 1/2000"
local range2 "if keep_scattered in 30001/34000"
local range3 "in -2000/l"
forvalues r = 1/3 {
    local sample `range`r''
    foreach method in ols ppml {
        local label `method'_short`r'
        foreach mode in serial forced parallel {
            local t = cond("`mode'" == "serial", 1, `workers')
            local injection = cond("`mode'" == "forced", "inject(serial_input)", "")
            _parallel_case `"`trace'"' `label'|`mode'
            if "`method'" == "ols" {
                if "`mode'" == "serial" {
                    quietly fereg y b f d `sample' [aw=1+w], absorb(sg g2) threads(`t')
                }
                else {
                    tempvar expression_weight
                    quietly generate double `expression_weight' = 1+w
                    quietly _fereg_estimate_ols_test y b f d `sample', ///
                        aweight(`expression_weight') absorb(sg g2) threads(`t') `injection'
                    drop `expression_weight'
                }
            }
            else {
                quietly _fereg_estimate_ppml py b f d `sample', ///
                    absorb(g2) exposure(expo) threads(`t') `injection'
            }
            if "`mode'" == "serial" {
                matrix short_b = e(b)
                generate byte short_sample = e(sample)
            }
            if "`mode'" == "forced" {
                matrix short_forced_b = e(b)
                matrix short_forced_V = e(V)
            }
            if "`mode'" == "parallel" {
                assert mreldif(e(b), short_forced_b) == 0
                assert mreldif(e(V), short_forced_V) == 0
                assert mreldif(e(b), short_b) < 1e-8
                assert e(sample) == short_sample
                drop short_sample
            }
            assert row == _n
        }
    }
}

* Variable order, sort order, preserve/restore and a non-default frame change
* nothing but which rows/variables the SDK maps; repeated calls stay exact.
_parallel_case `"`trace'"' ols_reordered|serial
quietly fereg y b i l f d if keep_scattered, absorb(g1) threads(1)
matrix order_b = e(b)
order d f l i b y, first
_parallel_case `"`trace'"' ols_reordered|parallel
quietly fereg y b i l f d if keep_scattered, absorb(g1) threads(`workers')
assert mreldif(e(b), order_b) < 1e-9
preserve
gsort -row
_parallel_case `"`trace'"' ols_sorted|serial
quietly fereg y b i l f d if keep_scattered, absorb(g1) threads(1)
matrix sorted_b = e(b)
_parallel_case `"`trace'"' ols_sorted|parallel
quietly fereg y b i l f d if keep_scattered, absorb(g1) threads(`workers')
assert mreldif(e(b), sorted_b) == 0
restore
assert row == _n

* Small storage segments: the SDK resolves rows across many segments.
tempfile segmented
quietly save `segmented'
local default_segment = c(segmentsize)
clear
set segmentsize 1m
quietly use `segmented'
_parallel_case `"`trace'"' ols_segments|serial
quietly fereg y b i l f d if keep_scattered, absorb(g1) threads(1)
matrix segment_b = e(b)
_parallel_case `"`trace'"' ols_segments|parallel
quietly fereg y b i l f d if keep_scattered, absorb(g1) threads(`workers')
assert mreldif(e(b), segment_b) < 1e-9
clear
set segmentsize `default_segment'
quietly use `segmented'
assert row == _n

frame copy default other
frame other {
    drop if mod(row, 5) == 0
    _parallel_case `"`trace'"' ols_frame|serial
    quietly fereg y b i l f d, absorb(g1) threads(1)
    matrix frame_b = e(b)
    _parallel_case `"`trace'"' ols_frame|parallel
    quietly fereg y b i l f d, absorb(g1) threads(`workers')
    assert mreldif(e(b), frame_b) < 1e-9
}
forvalues repeat = 1/3 {
    _parallel_case `"`trace'"' ols_repeat|parallel
    quietly fereg y b i l f d if keep_scattered, absorb(g1) threads(`workers')
    assert mreldif(e(b), order_b) < 1e-9
}

* Alias variables keep the serial reader (Stata 18+).
if c(stata_version) >= 18 {
    frame other: generate double z = 2 * y
    frlink 1:1 row, frame(other)
    fralias add z, from(other)
    _parallel_case `"`trace'"' ols_alias|alias
    quietly fereg y b f z, threads(`workers')
}

* The serial fallback also prepares packed input above 64 source columns.
clear
set seed 739391
set obs 16387
forvalues j=1/128 {
    quietly generate double x`j' = rnormal()
}
quietly generate int g = mod(_n,101)
quietly generate double y = x1 + .5*x2 + sin(g/11) + rnormal()
foreach k in 64 128 {
    quietly areg y x1-x`k', absorb(g)
    matrix wide_reference_b = e(b)
    matrix wide_reference_V = e(V)
    foreach mode in serial forced parallel {
        local t = cond("`mode'"=="serial",1,`workers')
        local inject = cond("`mode'"=="forced","inject(serial_input)","")
        _parallel_case `"`trace'"' ols_wide`k'|`mode'
        quietly _fereg_estimate_ols_test y x1-x`k', absorb(g) threads(`t') `inject'
        assert mreldif(e(b),wide_reference_b) < 1e-9
        assert mreldif(e(V),wide_reference_V) < 1e-8
        assert e(N) == _N
        if "`mode'"=="forced" {
            matrix wide_forced_b = e(b)
            matrix wide_forced_V = e(V)
        }
        if "`mode'"=="parallel" {
            assert mreldif(e(b),wide_forced_b) == 0
            assert mreldif(e(V),wide_forced_V) == 0
        }
    }
}

display as result "END_FIXTURE|parallel_input_oracle|rc=0"
