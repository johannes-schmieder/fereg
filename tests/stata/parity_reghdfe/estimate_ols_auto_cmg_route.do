version 18.0
clear all
set more off

quietly findfile fereg.ado
local ado_path = subinstr(`"`r(fn)'"', "\\", "/", .)
local suffix "/stata/fereg.ado"
assert substr(`"`ado_path'"', -strlen(`"`suffix'"'), .) == `"`suffix'"'

program define __frg_make_panel
    syntax, Periods(integer) Workers(integer) Firms(integer)
    clear
    set obs `workers'
    generate long worker = _n
    expand `periods'
    bysort worker: generate byte period = _n
    generate int firm = 1 + mod(worker + period, `firms')
    generate double x = mod((_n-1)*37 + 11, 997)/97 - 5
    generate double y = .7*x + sin(worker * .13) + cos(firm * .29) + ///
        (mod((_n-1)*53 + 7, 101) - 50)*.001
end

* Independent small-design oracle. Public Auto may choose its certified solver;
* geometry is not a contract for a particular numerical route code.
program define __frg_check_dummy
    syntax, Absorb(varlist) [Memory(string)]
    if "`memory'" == "" local memory fast
    tempvar auto_residual auto_sample reference_residual
    tempname auto_b auto_v auto_n auto_df_r auto_df_a
    quietly fereg y x, absorb(`absorb') solver(auto) memory(`memory') ///
        poolsize(1) threads(2) residuals(`auto_residual')
    assert e(converged) == 1
    assert e(fereg_certificate)[24,2] == 1
    assert e(sample) == 1
    scalar `auto_b' = _b[x]
    scalar `auto_v' = _se[x]^2
    scalar `auto_n' = e(N)
    scalar `auto_df_r' = e(df_r)
    scalar `auto_df_a' = e(df_a)
    generate byte `auto_sample' = e(sample)
    local dummies
    foreach fe of local absorb {
        local dummies `dummies' i.`fe'
    }
    quietly regress y x `dummies'
    assert e(sample) == `auto_sample'
    assert e(N) == `auto_n' & e(N) == _N
    assert e(df_r) == `auto_df_r'
    assert e(rank) - 1 == `auto_df_a'
    assert abs(_b[x] - `auto_b') <= 1e-10 + 1e-7*abs(_b[x])
    assert abs(_se[x]^2 - `auto_v') <= 1e-10 + 1e-6*_se[x]^2
    quietly predict double `reference_residual', residuals
    assert !missing(`auto_residual', `reference_residual')
    assert abs(`auto_residual' - `reference_residual') <= ///
        1e-9 + 1e-7*abs(`reference_residual')
end

* Retain the former AKM boundary geometry: N/G_high=3 and G_high/G_second=10.
__frg_make_panel, periods(3) workers(100) firms(10)
foreach mode in fast low {
    __frg_check_dummy, absorb(worker firm) memory(`mode')
}

* Shallow worker panels.
__frg_make_panel, periods(2) workers(100) firms(10)
__frg_check_dummy, absorb(worker firm)

* The neighboring graph-cardinality geometry.
__frg_make_panel, periods(10) workers(100) firms(11)
__frg_check_dummy, absorb(worker firm)

* Preserve both large disconnected-component geometries and their nonzero noise.
program define __frg_make_component_panel
    syntax, Components(integer) [Balanced]
    clear
    set obs 99000
    generate long obs_id = _n
    generate long worker = ceil(_n / 3)
    bysort worker: generate byte period = _n - 1
    local tiny_components = cond("`balanced'" == "", `components' - 1, `components')
    local small_workers = `tiny_components' * 30
    local small_firms = `tiny_components' * 3
    local remaining_firms = 3300 - `small_firms'
    generate long firm = floor((worker - 1) / 30) * 3 + ///
        mod(mod(worker - 1, 3) + period, 3) + 1 if worker <= `small_workers'
    if "`balanced'" == "" {
        replace firm = `small_firms' + ///
            mod(mod(worker - `small_workers' - 1, `remaining_firms') + period, ///
            `remaining_firms') + 1 if worker > `small_workers'
    }
    assert !missing(firm)
    generate double x = mod((obs_id-1)*37 + 11, 997)/97 - 5
    generate double y = .7*x + sin(worker * .13) + cos(firm * .29) + ///
        (mod((obs_id-1)*53 + 7, 101) - 50)*.001
end

* Secondary solver-agreement regression, not an independent oracle: avoid a
* huge dummy regression for 33,000 workers while retaining public certification
* and the supported explicit CMG request on each disconnected geometry.
program define __frg_check_cmg_agreement
    tempvar auto_residual auto_sample cmg_residual
    tempname auto_b auto_v auto_n auto_df_r auto_df_a
    quietly fereg y x, absorb(worker firm) solver(auto) threads(2) ///
        residuals(`auto_residual')
    assert e(converged) == 1
    assert e(fereg_certificate)[24,2] == 1
    assert e(sample) == 1
    scalar `auto_b' = _b[x]
    scalar `auto_v' = _se[x]^2
    scalar `auto_n' = e(N)
    scalar `auto_df_r' = e(df_r)
    scalar `auto_df_a' = e(df_a)
    generate byte `auto_sample' = e(sample)
    quietly fereg y x, absorb(worker firm) solver(cmg) threads(2) ///
        residuals(`cmg_residual')
    assert e(fereg_solver)[1,2] == 7
    assert e(converged) == 1
    assert e(fereg_certificate)[24,2] == 1
    assert e(sample) == `auto_sample'
    assert e(N) == `auto_n' & e(N) == _N
    assert e(df_r) == `auto_df_r' & e(df_a) == `auto_df_a'
    assert abs(_b[x] - `auto_b') <= 1e-10 + 1e-7*abs(_b[x])
    assert abs(_se[x]^2 - `auto_v') <= 1e-10 + 1e-6*_se[x]^2
    assert !missing(`auto_residual', `cmg_residual')
    assert abs(`auto_residual' - `cmg_residual') <= ///
        1e-9 + 1e-7*abs(`cmg_residual')
end

__frg_make_component_panel, components(800)
__frg_check_cmg_agreement

__frg_make_component_panel, components(1100) balanced
__frg_check_cmg_agreement

* Retain nine- and ten-column nuisance geometries, listing the low-cardinality
* FE first. Independent dummy regression checks science, not route selection.
__frg_make_panel, periods(8) workers(100) firms(10)
generate byte nuisance10 = 1 + mod(_n - 1, 10)
__frg_check_dummy, absorb(nuisance10 worker firm)

generate byte nuisance11 = 1 + mod(_n - 1, 11)
__frg_check_dummy, absorb(nuisance11 worker firm)

* Regression for tests/manual/simple_AKM.do: ordinary FE certification at the
* public tolerance was not strong enough for the downstream OLS score
* certificate. The CMG graph system now solves more tightly without changing
* the public reconstruction contract or adding a second fit.
clear
set seed 12345
set obs 10000
generate long worker = _n
expand 3
bysort worker: generate int year = 2000 + _n - 1
bysort worker (year): generate int firm = ceil(runiform()*500) if _n == 1
bysort worker (year): replace firm = ///
    cond(runiform() < .15, ceil(runiform()*500), firm[_n-1]) if _n > 1
bysort worker: generate double worker_fe = rnormal(0,.4) if _n == 1
bysort worker: replace worker_fe = worker_fe[1]
bysort firm: generate double firm_fe = rnormal(0,.2) if _n == 1
bysort firm: replace firm_fe = firm_fe[1]
generate double experience = year - 2000 + runiform()*5
generate double exp2 = experience^2
generate double y = 2 + worker_fe + firm_fe + .03*experience - ///
    .0005*exp2 + rnormal(0,.2)
quietly fereg y experience exp2, ///
    absorb(worker_saved=worker firm_saved=firm) solver(cmg)
assert e(fereg_solver)[1,2] == 7
assert e(fereg_solver)[1,11] == 0
assert !missing(worker_saved, firm_saved)

di as result "END_FIXTURE|estimate_ols_auto_cmg_route|rc=0"
