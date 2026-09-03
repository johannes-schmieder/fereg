version 18.0
clear all
set more off
set linesize 255

local mode : environment FEREG_PPML_SOLVER_KKT_MODE
local repository : environment FEREG_PPML_SOLVER_REPOSITORY
local dataset : environment FEREG_PPML_SOLVER_DATASET
local result : environment FEREG_PPML_SOLVER_RESULT
local state : environment FEREG_PPML_SOLVER_KKT_STATE
local plugin : environment FEREG_PPML_SOLVER_KKT_PLUGIN
local candidate_state : environment FEREG_PPML_SOLVER_KKT_CANDIDATE_STATE
local default_state : environment FEREG_PPML_SOLVER_KKT_DEFAULT_STATE
local strict_state : environment FEREG_PPML_SOLVER_KKT_STRICT_STATE
local arm : environment FEREG_PPML_SOLVER_ARM
local reference_root : environment FEREG_PPML_SOLVER_REFERENCE_ROOT
local stata_log : environment FEREG_PPML_SOLVER_STATA_LOG
local controls = real(`"`: environment FEREG_PPML_SOLVER_K'"')
local threads = real(`"`: environment FEREG_PPML_SOLVER_THREADS'"')
if strtrim(`"`stata_log'"') != "" log using `"`stata_log'"', text replace

if !inlist(`"`mode'"', "fit", "audit") {
    display as error "unknown public-state KKT mode"
    exit 198
}
if missing(`controls') | `controls' < 0 | missing(`threads') | `threads' <= 0 {
    display as error "invalid public-state KKT dimensions"
    exit 198
}

local control_list
if `controls' > 0 {
    forvalues index = 1/`controls' {
        local control_list `control_list' x`index'
    }
}

if `"`mode'"' == "fit" {
    if !inlist(`"`arm'"', "map", "ppmlhdfe", "ppmlhdfe_strict") {
        display as error "unknown public-state KKT fit arm"
        exit 198
    }
    if strtrim(`"`state'"') == "" | strtrim(`"`result'"') == "" {
        display as error "public-state KKT fit output is missing"
        exit 198
    }

    if `"`arm'"' == "map" {
        adopath ++ `"`repository'/stata"'
        adopath ++ `"`repository'/dist"'
        capture findfile feppml.ado
        if _rc exit _rc
        local found = subinstr(`"`r(fn)'"', "\", "/", .)
        local expected = subinstr(`"`repository'/stata/feppml.ado"', "\", "/", .)
        if `"`found'"' != `"`expected'"' exit 603
        local normalized_plugin = subinstr(`"`plugin'"', "\", "/", .)
        local plugin_prefix = subinstr(`"`repository'/dist/fereg_"', "\", "/", .)
        if strpos(`"`normalized_plugin'"', `"`plugin_prefix'"') != 1 | ///
                substr(`"`normalized_plugin'"', -7, .) != ".plugin" exit 603
        capture confirm file `"`normalized_plugin'"'
        if _rc exit _rc
    }
    else {
        adopath ++ `"`reference_root'/require/src"'
        adopath ++ `"`reference_root'/ftools/src"'
        adopath ++ `"`reference_root'/reghdfe/src"'
        adopath ++ `"`reference_root'/ppmlhdfe/src"'
        foreach package in require ftools reghdfe ppmlhdfe {
            capture findfile `package'.ado
            if _rc exit _rc
            local found = subinstr(`"`r(fn)'"', "\", "/", .)
            local expected = subinstr(`"`reference_root'/`package'/src/`package'.ado"', "\", "/", .)
            if `"`found'"' != `"`expected'"' exit 603
        }
        capture findfile lftools.mlib
        if _rc exit _rc
        local found = subinstr(`"`r(fn)'"', "\", "/", .)
        local expected = subinstr(`"`reference_root'/ppmlhdfe/src/l/lftools.mlib"', "\", "/", .)
        if `"`found'"' != `"`expected'"' exit 603
        local stata_processors = min(`threads', c(processors_mach), c(processors_lic))
        quietly set processors `stata_processors'
    }

    use `"`dataset'"', clear
    assert observation == _n
    assert y > 0 & y < .
    local absorb_list "g1 g2"
    capture confirm variable g3
    if !_rc local absorb_list "g1 g2 g3"
    tempvar fitted_mu reference_d

    if `"`arm'"' == "map" {
        capture noisily feppml y `control_list', absorb(`absorb_list') ///
            vce(robust) separation(none) memory(fast) threads(`threads') ///
            solver(map) mu(`fitted_mu')
    }
    else if `"`arm'"' == "ppmlhdfe" {
        capture noisily ppmlhdfe y `control_list', absorb(`absorb_list') ///
            vce(robust) separation(none) d(`reference_d') verbose(2)
        if !_rc capture noisily predict double `fitted_mu', mu
    }
    else {
        capture noisily ppmlhdfe y `control_list', absorb(`absorb_list') ///
            vce(robust) separation(none) tolerance(1e-10) ///
            itolerance(1e-12) d(`reference_d') verbose(2)
        if !_rc capture noisily predict double `fitted_mu', mu
    }
    local rc = _rc
    if `rc' {
        display as error "FEREG_PPML_SOLVER_KKT_FIT_FAIL|arm=`arm'|rc=`rc'"
        exit `rc'
    }
    assert `fitted_mu' > 0 & `fitted_mu' < . if e(sample)
    assert missing(`fitted_mu') if !e(sample)

    local sample_n = e(N)
    local ll = e(ll)
    local deviance = e(deviance)
    local converged = e(converged)
    local rank = e(rank)
    local df_a = e(df_a)
    local df_resid .
    capture local df_resid = e(df_resid)
    if _rc capture local df_resid = e(df_r)
    local route_code .
    local selected_engine_code .
    local effective_engine_code .
    local fallback_count .
    local effective_threads .
    local certificate_passed .
    local reference_acceleration
    local reference_transform
    if `"`arm'"' == "map" {
        local route_code = e(feppml_solver)[1,4]
        local selected_engine_code = e(feppml_solver)[1,3]
        local effective_engine_code = e(feppml_solver)[1,5]
        local fallback_count = e(feppml_solver_fallback_count)
        local effective_threads = e(feppml_threads)
        matrix __certificate = e(feppml_certificate)
        local certificate_passed = __certificate[16,1]
    }
    else {
        local reference_acceleration "conjugate_gradient"
        local reference_transform "symmetric_kaczmarz"
        display as text "FEREG_PPML_SOLVER_REFERENCE_ROUTE_PASS|arm=`arm'|acceleration=`reference_acceleration'|transform=`reference_transform'"
    }
    local b_names : colfullnames e(b)
    mata: st_local("b_values", invtokens(strofreal(st_matrix("e(b)"), "%21.17e")))

    file open output using `"`result'"', write text replace
    file write output "arm" _tab "N" _tab "ll" _tab "deviance" _tab ///
        "converged" _tab "rank" _tab "df_a" _tab "df_resid" _tab ///
        "route_code" _tab "selected_engine_code" _tab ///
        "effective_engine_code" _tab "fallback_count" _tab ///
        "effective_threads" _tab "certificate_passed" _tab ///
        "b_names" _tab "b_values" _tab "reference_acceleration" _tab ///
        "reference_transform" _n
    file write output `"`arm'"' _tab %21.17e (`sample_n') _tab ///
        %21.17e (`ll') _tab %21.17e (`deviance') _tab ///
        %21.17e (`converged') _tab %21.17e (`rank') _tab ///
        %21.17e (`df_a') _tab %21.17e (`df_resid') _tab ///
        %21.17e (`route_code') _tab %21.17e (`selected_engine_code') _tab ///
        %21.17e (`effective_engine_code') _tab %21.17e (`fallback_count') _tab ///
        %21.17e (`effective_threads') _tab %21.17e (`certificate_passed') _tab ///
        `"`b_names'"' _tab `"`b_values'"' _tab ///
        `"`reference_acceleration'"' _tab `"`reference_transform'"' _n
    file close output

    keep observation `fitted_mu'
    rename `fitted_mu' mu
    sort observation
    save `"`state'"', replace
    display as text "FEREG_PPML_SOLVER_KKT_FIT_PASS|arm=`arm'|N=`sample_n'"
    exit 0
}

if strtrim(`"`candidate_state'"') == "" | strtrim(`"`default_state'"') == "" | ///
        strtrim(`"`strict_state'"') == "" | strtrim(`"`result'"') == "" {
    display as error "public-state KKT audit input is missing"
    exit 198
}
use `"`dataset'"', clear
assert observation == _n
assert y > 0 & y < .
foreach state_arm in candidate default strict {
    merge 1:1 observation using `"``state_arm'_state'"', assert(3) nogen keepusing(mu)
    rename mu mu_`state_arm'
    assert mu_`state_arm' > 0 & mu_`state_arm' < .
}
local absorb_list "g1 g2"
capture confirm variable g3
if !_rc local absorb_list "g1 g2 g3"

mata:
real rowvector public_state_kkt(
    real colvector y,
    real matrix x,
    real matrix groups,
    real colvector mu)
{
    real scalar n, column, effect, row, level, max_level, score_scale
    real scalar finite_absolute, fe_absolute, relative_infinity, relative_two
    real scalar log_likelihood, deviance
    real matrix finite_design
    real colvector residual, moments, basis_squares, relative
    real colvector effect_moments, effect_basis_squares

    n = rows(y)
    residual = y - mu
    score_scale = max((
        sqrt(quadcross(residual, residual :/ mu)),
        sqrt(quadcross(y, y :/ mu))
    ))
    finite_design = x, J(n, 1, 1)
    moments = J(cols(finite_design), 1, 0)
    basis_squares = J(cols(finite_design), 1, 0)
    for (column = 1; column <= cols(finite_design); column++) {
        moments[column] = quadcross(finite_design[,column], residual)
        basis_squares[column] = quadcross(
            finite_design[,column], mu :* finite_design[,column]
        )
    }
    finite_absolute = max(abs(moments))
    fe_absolute = 0
    for (effect = 1; effect <= cols(groups); effect++) {
        max_level = max(groups[,effect])
        effect_moments = J(max_level, 1, 0)
        effect_basis_squares = J(max_level, 1, 0)
        for (row = 1; row <= n; row++) {
            level = groups[row,effect]
            effect_moments[level] = effect_moments[level] + residual[row]
            effect_basis_squares[level] = effect_basis_squares[level] + mu[row]
        }
        fe_absolute = max((fe_absolute, max(abs(effect_moments))))
        moments = moments \ effect_moments
        basis_squares = basis_squares \ effect_basis_squares
    }
    relative = abs(moments) :/ (sqrt(basis_squares) :* score_scale)
    relative_infinity = max(relative)
    relative_two = sqrt(quadcross(relative, relative) / rows(relative))
    log_likelihood = quadcross(
        J(n, 1, 1), y :* ln(mu) - mu - lngamma(y :+ 1)
    )
    deviance = quadcross(
        J(n, 1, 1), 2 :* (mu - y + y :* ln(y :/ mu))
    )
    return((
        finite_absolute, fe_absolute, relative_infinity, relative_two,
        score_scale, log_likelihood, deviance, min(mu), max(mu)
    ))
}

y = st_data(., "y")
control_names = tokens(st_local("control_list"))
x = cols(control_names) ? st_data(., control_names) : J(rows(y), 0, .)
groups = st_data(., tokens(st_local("absorb_list")))
audit = public_state_kkt(y, x, groups, st_data(., "mu_candidate")) \
    public_state_kkt(y, x, groups, st_data(., "mu_default")) \
    public_state_kkt(y, x, groups, st_data(., "mu_strict"))
st_matrix("PUBLIC_STATE_KKT", audit)
end

matrix rownames PUBLIC_STATE_KKT = map ppmlhdfe ppmlhdfe_strict
matrix colnames PUBLIC_STATE_KKT = finite_score_absinf fe_score_absinf ///
    score_relinf score_rel2 score_scale recomputed_ll recomputed_deviance ///
    min_mu max_mu
file open output using `"`result'"', write text replace
file write output "arm" _tab "finite_score_absinf" _tab "fe_score_absinf" _tab ///
    "score_relinf" _tab "score_rel2" _tab "score_scale" _tab ///
    "recomputed_ll" _tab "recomputed_deviance" _tab "min_mu" _tab "max_mu" _n
forvalues row = 1/3 {
    local output_arm = cond(`row' == 1, "map", cond(`row' == 2, "ppmlhdfe", "ppmlhdfe_strict"))
    file write output `"`output_arm'"'
    forvalues column = 1/9 {
        file write output _tab %21.17e (PUBLIC_STATE_KKT[`row',`column'])
    }
    file write output _n
}
file close output
display as text "FEREG_PPML_SOLVER_KKT_AUDIT_PASS|N=" _N
exit 0
