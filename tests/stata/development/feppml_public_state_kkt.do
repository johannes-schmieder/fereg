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
local projection_max_iterations = real(`"`: environment FEREG_PPML_SOLVER_KKT_PROJECTION_MAXITER'"')
local projection_refresh_interval = real(`"`: environment FEREG_PPML_SOLVER_KKT_PROJECTION_REFRESH'"')
local projection_normal_loose = real(`"`: environment FEREG_PPML_SOLVER_KKT_PROJECTION_NORMAL_LOOSE'"')
local projection_normal_final = real(`"`: environment FEREG_PPML_SOLVER_KKT_PROJECTION_NORMAL_FINAL'"')
local projection_normal_infinity = real(`"`: environment FEREG_PPML_SOLVER_KKT_PROJECTION_NORMAL_INFINITY'"')
local projection_recurrence_tolerance = real(`"`: environment FEREG_PPML_SOLVER_KKT_PROJECTION_RECURRENCE'"')
local projection_tail_tolerance = real(`"`: environment FEREG_PPML_SOLVER_KKT_PROJECTION_TAIL'"')
local projection_feasible_tolerance = real(`"`: environment FEREG_PPML_SOLVER_KKT_PROJECTION_FEASIBLE'"')
if strtrim(`"`stata_log'"') != "" log using `"`stata_log'"', text replace

if !inlist(`"`mode'"', "fit", "audit") {
    display as error "unknown public-state KKT mode"
    exit 198
}
if missing(`controls') | `controls' < 0 | missing(`threads') | `threads' <= 0 {
    display as error "invalid public-state KKT dimensions"
    exit 198
}
if `"`mode'"' == "audit" & ( ///
        missing(`projection_max_iterations') | `projection_max_iterations' < 1 | ///
        `projection_max_iterations' != floor(`projection_max_iterations') | ///
        missing(`projection_refresh_interval') | `projection_refresh_interval' < 1 | ///
        `projection_refresh_interval' != floor(`projection_refresh_interval') | ///
        missing(`projection_normal_loose') | `projection_normal_loose' <= 0 | ///
        missing(`projection_normal_final') | `projection_normal_final' <= 0 | ///
        `projection_normal_final' >= `projection_normal_loose' | ///
        missing(`projection_normal_infinity') | `projection_normal_infinity' <= 0 | ///
        missing(`projection_recurrence_tolerance') | `projection_recurrence_tolerance' <= 0 | ///
        missing(`projection_tail_tolerance') | `projection_tail_tolerance' <= 0 | ///
        missing(`projection_feasible_tolerance') | `projection_feasible_tolerance' <= 0) {
    display as error "invalid public-state model-space projection policy"
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
    local absorb_list "g1"
    capture confirm variable g2
    if !_rc local absorb_list "`absorb_list' g2"
    capture confirm variable g3
    if !_rc local absorb_list "`absorb_list' g3"
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
        if `"`absorb_list'"' == "g1" local reference_acceleration "none"
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
local absorb_list "g1"
capture confirm variable g2
if !_rc local absorb_list "`absorb_list' g2"
capture confirm variable g3
if !_rc local absorb_list "`absorb_list' g3"

mata:
struct fereg_kkt_model_space {
    real scalar status
    real scalar n
    real scalar controls
    real scalar effects
    real scalar levels1
    real scalar levels2
    real scalar levels3
    real scalar columns
    real matrix x
    real matrix groups
    real colvector order1
    real colvector order2
    real colvector order3
    real matrix panel1
    real matrix panel2
    real matrix panel3
    real colvector inverse_sqrt_count1
    real colvector inverse_sqrt_count2
    real colvector inverse_sqrt_count3
}

real matrix fereg_kkt__dense_groups(real matrix groups)
{
    real scalar effect, level, first, last
    real matrix out, panel
    real colvector permutation

    out = J(rows(groups), cols(groups), .)
    for (effect = 1; effect <= cols(groups); effect++) {
        permutation = order(groups[,effect], 1)
        panel = panelsetup(groups[permutation,effect], 1)
        for (level = 1; level <= rows(panel); level++) {
            first = panel[level,1]
            last = panel[level,2]
            out[permutation[|first \ last|],effect] = J(last - first + 1, 1, level)
        }
    }
    return(out)
}

struct fereg_kkt_model_space scalar fereg_kkt__prepare(
    real matrix x,
    real matrix groups)
{
    struct fereg_kkt_model_space scalar out
    real scalar column, scale
    real colvector counts

    out.status = 0
    out.n = rows(groups)
    out.controls = cols(x)
    out.effects = cols(groups)
    out.levels1 = 0
    out.levels2 = 0
    out.levels3 = 0
    out.columns = 0
    out.x = x
    out.groups = groups
    out.order1 = out.order2 = out.order3 = J(0, 1, .)
    out.panel1 = out.panel2 = out.panel3 = J(0, 2, .)
    out.inverse_sqrt_count1 = out.inverse_sqrt_count2 =
        out.inverse_sqrt_count3 = J(0, 1, .)
    if (out.n == 0 | rows(x) != out.n |
        (out.effects < 1 | out.effects > 3) |
        hasmissing(x) | hasmissing(groups)) {
        return(out)
    }
    for (column = 1; column <= out.controls; column++) {
        scale = sqrt(quadcross(out.x[,column], out.x[,column]))
        if (scale <= 0 | scale >= .) return(out)
        out.x[,column] = out.x[,column] :/ scale
    }
    if (any(groups :< 1) | any(groups :!= floor(groups))) return(out)
    out.groups = fereg_kkt__dense_groups(groups)

    out.order1 = order(out.groups[,1], 1)
    out.panel1 = panelsetup(out.groups[out.order1,1], 1)
    counts = panelsum(J(out.n, 1, 1), out.panel1)
    out.levels1 = rows(counts)
    if (any(counts :<= 0)) return(out)
    out.inverse_sqrt_count1 = 1 :/ sqrt(counts)

    if (out.effects >= 2) {
        out.order2 = order(out.groups[,2], 1)
        out.panel2 = panelsetup(out.groups[out.order2,2], 1)
        counts = panelsum(J(out.n, 1, 1), out.panel2)
        out.levels2 = rows(counts)
        if (any(counts :<= 0)) return(out)
        out.inverse_sqrt_count2 = 1 :/ sqrt(counts)
    }

    if (out.effects == 3) {
        out.order3 = order(out.groups[,3], 1)
        out.panel3 = panelsetup(out.groups[out.order3,3], 1)
        counts = panelsum(J(out.n, 1, 1), out.panel3)
        out.levels3 = rows(counts)
        if (any(counts :<= 0)) return(out)
        out.inverse_sqrt_count3 = 1 :/ sqrt(counts)
    }
    out.columns = out.controls + out.levels1 + out.levels2 + out.levels3
    if (out.columns <= 0) return(out)
    out.status = 1
    return(out)
}

real colvector fereg_kkt__forward(
    struct fereg_kkt_model_space scalar design,
    real colvector coefficient)
{
    real scalar first, last
    real colvector fitted, one_effect

    fitted = J(design.n, 1, 0)
    first = 1
    if (design.controls > 0) {
        last = design.controls
        fitted = fitted + design.x * coefficient[|first \ last|]
        first = last + 1
    }
    last = first + design.levels1 - 1
    one_effect = coefficient[|first \ last|] :* design.inverse_sqrt_count1
    fitted = fitted + one_effect[design.groups[,1]]
    first = last + 1
    if (design.effects >= 2) {
        last = first + design.levels2 - 1
        one_effect = coefficient[|first \ last|] :* design.inverse_sqrt_count2
        fitted = fitted + one_effect[design.groups[,2]]
        first = last + 1
    }
    if (design.effects == 3) {
        last = first + design.levels3 - 1
        one_effect = coefficient[|first \ last|] :* design.inverse_sqrt_count3
        fitted = fitted + one_effect[design.groups[,3]]
    }
    return(fitted)
}

real colvector fereg_kkt__transpose(
    struct fereg_kkt_model_space scalar design,
    real colvector value)
{
    real colvector out

    out = design.controls ? design.x' * value : J(0, 1, .)
    out = out \
        (panelsum(value[design.order1], design.panel1) :*
            design.inverse_sqrt_count1)
    if (design.effects >= 2) {
        out = out \
            (panelsum(value[design.order2], design.panel2) :*
                design.inverse_sqrt_count2)
    }
    if (design.effects == 3) {
        out = out \
            (panelsum(value[design.order3], design.panel3) :*
                design.inverse_sqrt_count3)
    }
    return(out)
}

real rowvector fereg_kkt__project(
    real colvector y,
    real colvector eta,
    struct fereg_kkt_model_space scalar design,
    real scalar max_iterations,
    real scalar refresh_interval,
    real scalar normal_loose,
    real scalar normal_final,
    real scalar normal_infinity,
    real scalar recurrence_tolerance,
    real scalar tail_tolerance,
    real scalar feasible_tolerance)
{
    real scalar algorithm, status, iteration, denominator, alpha, beta
    real scalar gamma, gamma_new, eta_scale, fresh_normal_two
    real scalar fresh_normal_infinity, recurrence_relative, maximum_recurrence
    real scalar loose_saved, converged, breakdown, residual_rms
    real scalar residual_relative, tail_relative
    real scalar projected_ll, projected_deviance
    real colvector coefficient, residual, normal, direction, action
    real colvector fitted, fresh_residual, fresh_normal, loose_fitted
    real colvector projected_mu

    algorithm = 1
    status = 3
    iteration = 0
    breakdown = 0
    eta_scale = max((1, sqrt(mean(eta:^2))))
    maximum_recurrence = 0
    loose_saved = 0
    converged = 0
    residual_rms = residual_relative = tail_relative = .
    fresh_normal_two = fresh_normal_infinity = .
    projected_ll = projected_deviance = .
    fitted = fresh_residual = fresh_normal = loose_fitted = J(rows(eta), 1, .)
    projected_mu = J(rows(eta), 1, .)
    if (design.status != 1 | rows(y) != design.n | rows(eta) != design.n |
        hasmissing(y) | hasmissing(eta)) {
        breakdown = 1
        return((algorithm, status, iteration, design.columns, eta_scale,
            ., ., ., ., ., ., maximum_recurrence, breakdown,
            projected_ll, projected_deviance, ., .))
    }

    coefficient = J(design.columns, 1, 0)
    residual = eta
    normal = fereg_kkt__transpose(design, residual)
    direction = normal
    gamma = quadcross(normal, normal)
    if (gamma >= .) breakdown = 3
    for (iteration = 1; iteration <= max_iterations & breakdown == 0; iteration++) {
        if (gamma == 0) {
            iteration = iteration - 1
            fitted = fereg_kkt__forward(design, coefficient)
            fresh_residual = eta - fitted
            fresh_normal = fereg_kkt__transpose(design, fresh_residual)
            loose_fitted = fitted
            loose_saved = 1
            converged = 1
            break
        }
        action = fereg_kkt__forward(design, direction)
        denominator = quadcross(action, action)
        if (denominator <= 0 | denominator >= .) {
            breakdown = 2
            break
        }
        alpha = gamma / denominator
        if (alpha >= .) {
            breakdown = 3
            break
        }
        coefficient = coefficient + alpha :* direction
        residual = residual - alpha :* action
        normal = fereg_kkt__transpose(design, residual)
        gamma_new = quadcross(normal, normal)
        if (gamma_new >= . | hasmissing(coefficient) | hasmissing(residual)) {
            breakdown = 3
            break
        }

        if (mod(iteration, refresh_interval) == 0 |
            (!loose_saved &
                sqrt(gamma_new / design.columns) / eta_scale <= normal_loose) |
            sqrt(gamma_new / design.columns) / eta_scale <= normal_final) {
            fitted = fereg_kkt__forward(design, coefficient)
            fresh_residual = eta - fitted
            fresh_normal = fereg_kkt__transpose(design, fresh_residual)
            recurrence_relative = sqrt(mean((fresh_residual - residual):^2)) /
                eta_scale
            maximum_recurrence = max((maximum_recurrence, recurrence_relative))
            if (recurrence_relative > recurrence_tolerance |
                hasmissing(fitted) | hasmissing(fresh_normal)) {
                breakdown = 4
                break
            }
            fresh_normal_two = sqrt(
                quadcross(fresh_normal, fresh_normal) / design.columns
            ) / eta_scale
            fresh_normal_infinity = max(abs(fresh_normal)) / eta_scale
            residual = fresh_residual
            normal = fresh_normal
            gamma_new = quadcross(normal, normal)
            if (!loose_saved & fresh_normal_two <= normal_loose) {
                loose_fitted = fitted
                loose_saved = 1
            }
            if (fresh_normal_two <= normal_final &
                fresh_normal_infinity <= normal_infinity) {
                converged = 1
                break
            }
        }
        if (gamma_new == 0) {
            gamma = 0
            direction = normal
        }
        else {
            beta = gamma_new / gamma
            if (beta >= .) {
                breakdown = 3
                break
            }
            direction = normal + beta :* direction
            gamma = gamma_new
        }
    }
    if (!converged & breakdown == 0) {
        breakdown = 5
        iteration = max_iterations
    }
    if (breakdown != 0) {
        return((algorithm, status, iteration, design.columns, eta_scale,
            ., ., ., ., ., ., maximum_recurrence, breakdown,
            projected_ll, projected_deviance, ., .))
    }

    fitted = fereg_kkt__forward(design, coefficient)
    fresh_residual = eta - fitted
    fresh_normal = fereg_kkt__transpose(design, fresh_residual)
    fresh_normal_two = sqrt(
        quadcross(fresh_normal, fresh_normal) / design.columns
    ) / eta_scale
    fresh_normal_infinity = max(abs(fresh_normal)) / eta_scale
    residual_rms = sqrt(mean(fresh_residual:^2))
    residual_relative = residual_rms / eta_scale
    if (!loose_saved) loose_fitted = fitted
    tail_relative = sqrt(mean((fitted - loose_fitted):^2)) / eta_scale
    if (fresh_normal_two > normal_final |
        fresh_normal_infinity > normal_infinity |
        tail_relative > tail_tolerance) {
        breakdown = 6
        status = 3
    }
    else if (residual_relative <= feasible_tolerance) status = 1
    else status = 3

    projected_mu = exp(fitted)
    if (hasmissing(projected_mu) | any(projected_mu :<= 0)) {
        breakdown = 7
        status = 3
    }
    else {
        projected_ll = quadcross(
            J(design.n, 1, 1), y :* fitted - projected_mu - lngamma(y :+ 1)
        )
        projected_deviance = quadcross(
            J(design.n, 1, 1),
            2 :* (projected_mu - y + y :* (ln(y) - fitted))
        )
        if (projected_ll >= . | projected_deviance < 0 |
            projected_deviance >= .) {
            breakdown = 7
            status = 3
        }
    }
    return((algorithm, status, iteration, design.columns, eta_scale,
        max(abs(fresh_residual)), residual_rms, residual_relative,
        tail_relative, fresh_normal_two,
        fresh_normal_infinity, maximum_recurrence, breakdown,
        projected_ll, projected_deviance, min(projected_mu), max(projected_mu)))
}

real rowvector public_state_kkt(
    real colvector y,
    real matrix x,
    real matrix groups,
    real colvector mu,
    struct fereg_kkt_model_space scalar model_space,
    real rowvector projection_policy)
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
        score_scale, log_likelihood, deviance, min(mu), max(mu),
        fereg_kkt__project(
            y, ln(mu), model_space,
            projection_policy[1], projection_policy[2], projection_policy[3],
            projection_policy[4], projection_policy[5], projection_policy[6],
            projection_policy[7], projection_policy[8]
        )
    ))
}

y = st_data(., "y")
control_names = tokens(st_local("control_list"))
x = cols(control_names) ? st_data(., control_names) : J(rows(y), 0, .)
groups = st_data(., tokens(st_local("absorb_list")))
model_space = fereg_kkt__prepare(x, groups)
groups = model_space.groups
projection_policy = (
    strtoreal(st_local("projection_max_iterations")),
    strtoreal(st_local("projection_refresh_interval")),
    strtoreal(st_local("projection_normal_loose")),
    strtoreal(st_local("projection_normal_final")),
    strtoreal(st_local("projection_normal_infinity")),
    strtoreal(st_local("projection_recurrence_tolerance")),
    strtoreal(st_local("projection_tail_tolerance")),
    strtoreal(st_local("projection_feasible_tolerance"))
)
audit = public_state_kkt(
        y, x, groups, st_data(., "mu_candidate"), model_space, projection_policy
    ) \
    public_state_kkt(
        y, x, groups, st_data(., "mu_default"), model_space, projection_policy
    ) \
    public_state_kkt(
        y, x, groups, st_data(., "mu_strict"), model_space, projection_policy
    )
st_matrix("PUBLIC_STATE_KKT", audit)
end

matrix rownames PUBLIC_STATE_KKT = map ppmlhdfe ppmlhdfe_strict
matrix colnames PUBLIC_STATE_KKT = finite_score_absinf fe_score_absinf ///
    score_relinf score_rel2 score_scale recomputed_ll recomputed_deviance ///
    min_mu max_mu modelspace_algorithm_code modelspace_status_code ///
    modelspace_iterations modelspace_columns modelspace_eta_scale ///
    modelspace_resid_maxabs modelspace_resid_rms modelspace_resid_relrms ///
    modelspace_tail_relrms modelspace_normal_rel2 ///
    modelspace_normal_relinf modelspace_recurrence_relrms ///
    modelspace_breakdown_code projected_ll projected_deviance ///
    projected_min_mu projected_max_mu
file open output using `"`result'"', write text replace
file write output "arm" _tab "finite_score_absinf" _tab "fe_score_absinf" _tab ///
    "score_relinf" _tab "score_rel2" _tab "score_scale" _tab ///
    "recomputed_ll" _tab "recomputed_deviance" _tab "min_mu" _tab "max_mu" _tab ///
    "modelspace_algorithm_code" _tab "modelspace_status_code" _tab ///
    "modelspace_iterations" _tab "modelspace_columns" _tab ///
    "modelspace_eta_scale" _tab "modelspace_resid_maxabs" _tab ///
    "modelspace_resid_rms" _tab "modelspace_resid_relrms" _tab ///
    "modelspace_tail_relrms" _tab ///
    "modelspace_normal_rel2" _tab "modelspace_normal_relinf" _tab ///
    "modelspace_recurrence_relrms" _tab "modelspace_breakdown_code" _tab ///
    "projected_ll" _tab "projected_deviance" _tab ///
    "projected_min_mu" _tab "projected_max_mu" _n
forvalues row = 1/3 {
    local output_arm = cond(`row' == 1, "map", cond(`row' == 2, "ppmlhdfe", "ppmlhdfe_strict"))
    file write output `"`output_arm'"'
    forvalues column = 1/26 {
        file write output _tab %21.17e (PUBLIC_STATE_KKT[`row',`column'])
    }
    file write output _n
}
file close output
display as text "FEREG_PPML_SOLVER_KKT_AUDIT_PASS|N=" _N
exit 0
