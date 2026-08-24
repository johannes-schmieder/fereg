version 18.0
args capture_file manifest_sha256
set more off

if `"`capture_file'"' == "" | `"`manifest_sha256'"' == "" exit 198

capture program drop __frg_probe_matrix
program define __frg_probe_matrix
    args case_id matrix_name
    capture matrix __frg_probe_matrix_value = e(`matrix_name')
    if _rc {
        file write __frg_probe_capture "FEREG_FE_INTERACTION_PROBE|absent|`case_id'|matrix|`matrix_name'" _n
        exit
    }
    local rows = rowsof(__frg_probe_matrix_value)
    local cols = colsof(__frg_probe_matrix_value)
    file write __frg_probe_capture "FEREG_FE_INTERACTION_PROBE|matrix_dimensions|`case_id'|`matrix_name'|`rows'|`cols'" _n
    if `rows' == 0 | `cols' == 0 exit
    local rownames : rowfullnames __frg_probe_matrix_value
    local colnames : colfullnames __frg_probe_matrix_value
    forvalues row = 1/`rows' {
        local rowname : word `row' of `rownames'
        forvalues col = 1/`cols' {
            local colname : word `col' of `colnames'
            local value : display %21x __frg_probe_matrix_value[`row',`col']
            file write __frg_probe_capture "FEREG_FE_INTERACTION_PROBE|matrix|`case_id'|`matrix_name'|`row'|`col'|`rowname'|`colname'|`value'" _n
        }
    }
end

capture program drop __frg_probe_emit
program define __frg_probe_emit
    args case_id return_code
    file write __frg_probe_capture "FEREG_FE_INTERACTION_PROBE|case_begin|`case_id'" _n
    file write __frg_probe_capture "FEREG_FE_INTERACTION_PROBE|return_code|`case_id'|`return_code'" _n
    if `return_code' == 0 {
        foreach name in N N_full num_singletons df_m df_r df_a N_hdfe N_clust N_clustervars N_clust1 sumweights {
            local enames : e(scalars)
            local present : list posof "`name'" in enames
            if `present' == 0 {
                file write __frg_probe_capture "FEREG_FE_INTERACTION_PROBE|absent|`case_id'|scalar|`name'" _n
            }
            else {
                local value : display %21x e(`name')
                file write __frg_probe_capture "FEREG_FE_INTERACTION_PROBE|scalar|`case_id'|`name'|`value'" _n
            }
        }
        foreach name in absvars cmdline clustvar vce vcetype wtype wexp {
            local enames : e(macros)
            local present : list posof "`name'" in enames
            if `present' == 0 {
                file write __frg_probe_capture "FEREG_FE_INTERACTION_PROBE|absent|`case_id'|macro|`name'" _n
            }
            else {
                local text `"`e(`name')'"'
                local text = subinstr(`"`text'"', "%", "%25", .)
                local text = subinstr(`"`text'"', "|", "%7C", .)
                file write __frg_probe_capture "FEREG_FE_INTERACTION_PROBE|macro|`case_id'|`name'|`text'" _n
            }
        }
        __frg_probe_matrix `case_id' b
        __frg_probe_matrix `case_id' V
        __frg_probe_matrix `case_id' dof_table

        generate byte __frg_probe_sample = e(sample)
        capture confirm variable __frg_probe_resid
        local has_residual = (_rc == 0)
        file write __frg_probe_capture "FEREG_FE_INTERACTION_PROBE|observation_count|`case_id'|`=_N'" _n
        forvalues obs = 1/`=_N' {
            local obs_id : display %21x obs_id[`obs']
            local sample : display %21x __frg_probe_sample[`obs']
            file write __frg_probe_capture "FEREG_FE_INTERACTION_PROBE|observation|`case_id'|`obs_id'|sample|`sample'" _n
            if `has_residual' {
                local residual : display %21x __frg_probe_resid[`obs']
                file write __frg_probe_capture "FEREG_FE_INTERACTION_PROBE|observation|`case_id'|`obs_id'|residual|`residual'" _n
            }
        }
    }
    file write __frg_probe_capture "FEREG_FE_INTERACTION_PROBE|case_end|`case_id'" _n
end

capture program drop __frg_probe_target
program define __frg_probe_target
    import delimited using "target.csv", clear varnames(1) case(preserve)
end

capture program drop __frg_probe_cascade
program define __frg_probe_cascade
    clear
    set obs 12
    generate long obs_id = 100 + _n
    generate double y = real(word("3.20 4.05 4.60 5.25 5.90 6.35 7.10 7.85 8.10 8.75 9.30 10.05", _n))
    generate double x = real(word("-1.40 -0.90 -0.45 0.05 0.40 0.85 1.15 1.65 -1.10 -0.25 0.70 1.90", _n))
    generate int worker = real(word("1 1 1 1 2 2 2 2 3 3 4 4", _n))
    generate int firm = real(word("10 10 20 20 10 10 20 20 10 30 30 40", _n))
    generate int year = real(word("2020 2020 2021 2021 2020 2020 2021 2021 2020 2022 2022 2023", _n))
    generate int cluster_u = real(word("1 2 1 2 1 2 1 2 1 2 1 2", _n))
    generate int cluster_v = real(word("1 2 2 1 2 1 1 2 1 1 2 2", _n))
    generate int fw = real(word("1 1 1 1 1 1 1 1 1 1 1 2", _n))
end

capture program drop __frg_probe_missing
program define __frg_probe_missing
    clear
    set obs 12
    generate long obs_id = 200 + _n
    generate double y = real(word("3.10 3.90 4.40 5.20 5.80 6.50 7.10 7.90 8.40 9.00 . 9.80", _n))
    generate double x = real(word("-1.20 -0.70 -0.20 0.30 0.80 1.30 1.80 2.30 -1.00 -0.50 0.00 0.50", _n))
    generate int worker = real(word("1 1 1 1 2 2 2 2 3 3 3 3", _n))
    generate int firm = real(word("10 10 20 20 10 10 20 20 . 30 30 30", _n))
    generate int year = real(word("2020 2020 2021 2021 2020 2020 2021 2021 2020 . 2022 2022", _n))
end

capture program drop __frg_probe_rebase
program define __frg_probe_rebase
    clear
    set obs 10
    generate long obs_id = _n
    generate byte a = real(word("1 2 2 2 2 2 2 2 2 3", _n))
    generate byte b = real(word("1 1 1 1 1 2 2 2 2 3", _n))
    generate byte f = real(word("1 2 2 3 3 2 2 3 3 1", _n))
    generate byte z = real(word("1 1 2 1 2 1 2 1 2 2", _n))
    generate double x = real(word("-1.5 -1.0 -0.4 0.2 0.9 -0.8 -0.1 0.5 1.2 1.7", _n))
    generate double y = 1.4*x + 0.7*(f == 3) - 0.3*(z == 2) + ///
        0.2*(f == 3)*(z == 2) + 0.1*a - 0.2*b
end

file open __frg_probe_capture using `"`capture_file'"', write text replace
file write __frg_probe_capture "FEREG_FE_INTERACTION_PROBE|schema|1" _n
file write __frg_probe_capture "FEREG_FE_INTERACTION_PROBE|manifest_sha256|`manifest_sha256'" _n
file write __frg_probe_capture "FEREG_FE_INTERACTION_PROBE|environment|stata_version|`c(stata_version)'" _n
file write __frg_probe_capture "FEREG_FE_INTERACTION_PROBE|environment|stata_flavor|`c(flavor)'" _n
file write __frg_probe_capture "FEREG_FE_INTERACTION_PROBE|environment|stata_processors|`c(processors)'" _n
file write __frg_probe_capture "FEREG_FE_INTERACTION_PROBE|environment|stata_processors_limit|`c(processors_lic)'" _n
file write __frg_probe_capture "FEREG_FE_INTERACTION_PROBE|environment|os|`c(os)'" _n
file write __frg_probe_capture "FEREG_FE_INTERACTION_PROBE|environment|machine_type|`c(machine_type)'" _n

__frg_probe_target
capture noisily reghdfe y c.x1, absorb(worker firm#year) residuals(__frg_probe_resid)
local rc = _rc
__frg_probe_emit target_default_firm_year `rc'

__frg_probe_target
capture noisily reghdfe y c.x1, absorb(worker firm#year) keepsingletons residuals(__frg_probe_resid)
local rc = _rc
__frg_probe_emit target_keepsingletons `rc'

__frg_probe_cascade
capture noisily reghdfe y c.x, absorb(worker firm#year) residuals(__frg_probe_resid)
local rc = _rc
__frg_probe_emit tuple_second_fe_cascade `rc'

__frg_probe_cascade
capture noisily reghdfe y c.x [fw=fw], absorb(worker firm#year) residuals(__frg_probe_resid)
local rc = _rc
__frg_probe_emit fweight_frequency_cascade `rc'

__frg_probe_missing
capture noisily reghdfe y c.x, absorb(worker firm#year) keepsingletons residuals(__frg_probe_resid)
local rc = _rc
__frg_probe_emit missing_interaction_components `rc'

__frg_probe_target
capture noisily reghdfe y c.x1, absorb(worker firm#year firm#year) residuals(__frg_probe_resid)
local rc = _rc
__frg_probe_emit repeated_identical_interaction `rc'

__frg_probe_target
capture noisily reghdfe y c.x1, absorb(worker year#firm) residuals(__frg_probe_resid)
local rc = _rc
__frg_probe_emit interaction_order_year_firm `rc'

__frg_probe_target
capture noisily reghdfe y c.x1, absorb(worker firm#year) dof(none) residuals(__frg_probe_resid)
local rc = _rc
__frg_probe_emit dof_none `rc'

__frg_probe_target
capture noisily reghdfe y c.x1, absorb(worker firm#year) dof(firstpair) residuals(__frg_probe_resid)
local rc = _rc
__frg_probe_emit dof_firstpair `rc'

__frg_probe_target
capture noisily reghdfe y c.x1, absorb(worker firm#year) vce(cluster firm) residuals(__frg_probe_resid)
local rc = _rc
__frg_probe_emit nested_simple_cluster `rc'

__frg_probe_target
capture noisily reghdfe y c.x1, absorb(worker firm#year) vce(cluster firm#year) residuals(__frg_probe_resid)
local rc = _rc
__frg_probe_emit identical_interaction_cluster `rc'

__frg_probe_cascade
capture noisily reghdfe y c.x, absorb(worker firm#year) vce(cluster cluster_u#cluster_v) residuals(__frg_probe_resid)
local rc = _rc
__frg_probe_emit unrelated_interaction_cluster `rc'

__frg_probe_rebase
capture noisily reghdfe y c.x i.f, absorb(a#b) residuals(__frg_probe_resid)
local rc = _rc
__frg_probe_emit final_sample_default_factor_rebase `rc'

__frg_probe_rebase
capture noisily reghdfe y c.x i.f##i.z, absorb(a#b) residuals(__frg_probe_resid)
local rc = _rc
__frg_probe_emit final_sample_default_interaction_rebase `rc'

file write __frg_probe_capture "END_FIXTURE|reghdfe_fe_interaction_probe|rc=0" _n
file close __frg_probe_capture
