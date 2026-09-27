version 18.0
clear all
set more off

local staged_root = subinstr(strtrim(`"$FEREG_STAGED_PACKAGE_ROOT"'), "\\", "/", .)
local staged_plugin = strtrim(`"$FEREG_STAGED_PLUGIN_NAME"')
if `"`staged_root'"' == "" | `"`staged_plugin'"' == "" {
    display as error "feppml installation smoke requires an extracted release binding"
    exit 498
}
if substr(`"`staged_root'"', strlen(`"`staged_root'"'), 1) == "/" ///
    local staged_root = substr(`"`staged_root'"', 1, strlen(`"`staged_root'"') - 1)

foreach file in feppml.ado _fereg_ppml.ado _fereg_estimate_ppml.ado ///
    feppml_p.ado fereg.pkg stata.toc `staged_plugin' {
    capture findfile `file'
    if _rc {
        display as error "staged release does not resolve `file'"
        exit 601
    }
    local resolved = subinstr(`"`r(fn)'"', "\\", "/", .)
    if `"`resolved'"' != `"`staged_root'/`file'"' {
        display as error "`file' resolved outside the extracted release"
        exit 498
    }
}

foreach command in feppml _fereg_ppml _fereg_estimate_ppml feppml_p {
    capture which `command'
    local which_rc = _rc
    if `which_rc' exit `which_rc'
}

set obs 18
generate long obs_id = _n
generate byte x = _n > 9
generate double y = cond(x, 2 + mod(_n, 5), 1 + mod(_n, 3))

feppml y c.x, separation(none) memory(fast) threads(1) deterministic ///
    xb(__install_xb) d(__install_d) eta(__install_eta) mu(__install_mu)

assert `"`e(cmd)'"' == "feppml"
assert `"`e(predict)'"' == "feppml_p"
assert `"`e(feppml_gate)'"' == "public_ppml_v1"
assert `"`e(feppml_action_protocol)'"' == "1.5"
assert `"`e(feppml_model_wire)'"' == "2.1"
assert `"`e(feppml_request_wire)'"' == "1.7"
assert `"`e(feppml_result_wire)'"' == "1.4"
assert `"`e(solver)'"' == "auto"
assert `"`e(feppml_solver_selected)'"' == "map"
assert `"`e(feppml_solver_engine)'"' == "map"
assert `"`e(feppml_solver_effective)'"' == "map"
assert `"`e(feppml_solver_checkpoint)'"' == "map"
assert e(feppml_solver_fallback_count) == 0
assert e(feppml_linear_route) == 2
assert e(N) == 18
assert e(N_physical) == 18
assert e(N_effective) == 18
assert e(rank) == 2
assert e(df_m) == 1
assert e(df_a) == 0
assert e(converged) == 1
assert e(sample) == 1
assert abs(__install_d) <= 1e-12
assert abs(__install_eta - __install_xb) <= 1e-10
assert abs(__install_mu - exp(__install_eta)) <= 1e-10 * max(__install_mu, 1)
assert rowsof(e(feppml_status)) == 2
assert colsof(e(feppml_status)) == 4
assert rowsof(e(feppml_fit)) == 27
assert rowsof(e(feppml_dof)) == 12
assert rowsof(e(feppml_runtime)) == 108
assert rowsof(e(feppml_certificate)) == 16
assert rowsof(e(feppml_inference)) == 16
assert rowsof(e(feppml_drop_counts)) == 14
assert rowsof(e(feppml_separation_sequence)) == 5
assert rowsof(e(feppml_solver)) == 4
assert colsof(e(feppml_solver)) == 22
assert colsof(e(feppml_receipt)) == 33
assert e(feppml_receipt)[1,20] == 16
assert e(feppml_receipt)[1,21] == 14
assert e(feppml_receipt)[1,22] == 5
assert e(feppml_receipt)[1,33] == 1

predict double __install_predict
assert abs(__install_predict - __install_mu) <= 1e-12

display as result "END_FIXTURE|feppml_installation_smoke|rc=0"
