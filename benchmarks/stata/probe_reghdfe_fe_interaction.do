version 18.0
args capture_file manifest_sha256
set more off

if `"`capture_file'"' == "" | `"`manifest_sha256'"' == "" exit 198

capture program drop __stfx_probe_matrix
program define __stfx_probe_matrix
    args case_id matrix_name
    capture matrix __stfx_probe_matrix_value = e(`matrix_name')
    if _rc {
        file write __stfx_probe_capture "STFIXEST_FE_INTERACTION_PROBE|absent|`case_id'|matrix|`matrix_name'" _n
        exit
    }
    local rows = rowsof(__stfx_probe_matrix_value)
    local cols = colsof(__stfx_probe_matrix_value)
    file write __stfx_probe_capture "STFIXEST_FE_INTERACTION_PROBE|matrix_dimensions|`case_id'|`matrix_name'|`rows'|`cols'" _n
    if `rows' == 0 | `cols' == 0 exit
    local rownames : rowfullnames __stfx_probe_matrix_value
    local colnames : colfullnames __stfx_probe_matrix_value
    forvalues row = 1/`rows' {
        local rowname : word `row' of `rownames'
        forvalues col = 1/`cols' {
            local colname : word `col' of `colnames'
            local value : display %21x __stfx_probe_matrix_value[`row',`col']
            file write __stfx_probe_capture "STFIXEST_FE_INTERACTION_PROBE|matrix|`case_id'|`matrix_name'|`row'|`col'|`rowname'|`colname'|`value'" _n
        }
    }
end

capture program drop __stfx_probe_emit
program define __stfx_probe_emit
    args case_id return_code
    file write __stfx_probe_capture "STFIXEST_FE_INTERACTION_PROBE|case_begin|`case_id'" _n
    file write __stfx_probe_capture "STFIXEST_FE_INTERACTION_PROBE|return_code|`case_id'|`return_code'" _n
    if `return_code' == 0 {
        foreach name in N N_full num_singletons df_m df_r df_a N_hdfe N_clust N_clustervars N_clust1 sumweights {
            local enames : e(scalars)
            local present : list posof "`name'" in enames
            if `present' == 0 {
                file write __stfx_probe_capture "STFIXEST_FE_INTERACTION_PROBE|absent|`case_id'|scalar|`name'" _n
            }
            else {
                local value : display %21x e(`name')
                file write __stfx_probe_capture "STFIXEST_FE_INTERACTION_PROBE|scalar|`case_id'|`name'|`value'" _n
            }
        }
        foreach name in absvars cmdline clustvar vce vcetype wtype wexp {
            local enames : e(macros)
            local present : list posof "`name'" in enames
            if `present' == 0 {
                file write __stfx_probe_capture "STFIXEST_FE_INTERACTION_PROBE|absent|`case_id'|macro|`name'" _n
            }
            else {
                local text `"`e(`name')'"'
                local text = subinstr(`"`text'"', "%", "%25", .)
                local text = subinstr(`"`text'"', "|", "%7C", .)
                file write __stfx_probe_capture "STFIXEST_FE_INTERACTION_PROBE|macro|`case_id'|`name'|`text'" _n
            }
        }
        __stfx_probe_matrix `case_id' b
        __stfx_probe_matrix `case_id' V
        __stfx_probe_matrix `case_id' dof_table

        generate byte __stfx_probe_sample = e(sample)
        capture confirm variable __stfx_probe_resid
        local has_residual = (_rc == 0)
        file write __stfx_probe_capture "STFIXEST_FE_INTERACTION_PROBE|observation_count|`case_id'|`=_N'" _n
        forvalues obs = 1/`=_N' {
            local obs_id : display %21x obs_id[`obs']
            local sample : display %21x __stfx_probe_sample[`obs']
            file write __stfx_probe_capture "STFIXEST_FE_INTERACTION_PROBE|observation|`case_id'|`obs_id'|sample|`sample'" _n
            if `has_residual' {
                local residual : display %21x __stfx_probe_resid[`obs']
                file write __stfx_probe_capture "STFIXEST_FE_INTERACTION_PROBE|observation|`case_id'|`obs_id'|residual|`residual'" _n
            }
        }
    }
    file write __stfx_probe_capture "STFIXEST_FE_INTERACTION_PROBE|case_end|`case_id'" _n
end

capture program drop __stfx_probe_target
program define __stfx_probe_target
    import delimited using "target.csv", clear varnames(1) case(preserve)
end

capture program drop __stfx_probe_cascade
program define __stfx_probe_cascade
    clear
    input long obs_id double(y x) int(worker firm year cluster_u cluster_v fw)
        101  3.20 -1.40  1 10 2020 1 1 1
        102  4.05 -0.90  1 10 2020 2 2 1
        103  4.60 -0.45  1 20 2021 1 2 1
        104  5.25  0.05  1 20 2021 2 1 1
        105  5.90  0.40  2 10 2020 1 2 1
        106  6.35  0.85  2 10 2020 2 1 1
        107  7.10  1.15  2 20 2021 1 1 1
        108  7.85  1.65  2 20 2021 2 2 1
        109  8.10 -1.10  3 10 2020 1 1 1
        110  8.75 -0.25  3 30 2022 2 1 1
        111  9.30  0.70  4 30 2022 1 2 1
        112 10.05  1.90  4 40 2023 2 2 2
    end
end

capture program drop __stfx_probe_missing
program define __stfx_probe_missing
    clear
    input long obs_id double(y x) int(worker firm year)
        201 3.10 -1.20 1 10 2020
        202 3.90 -0.70 1 10 2020
        203 4.40 -0.20 1 20 2021
        204 5.20  0.30 1 20 2021
        205 5.80  0.80 2 10 2020
        206 6.50  1.30 2 10 2020
        207 7.10  1.80 2 20 2021
        208 7.90  2.30 2 20 2021
        209 8.40 -1.00 3  . 2020
        210 9.00 -0.50 3 30    .
        211    .  0.00 3 30 2022
        212 9.80  0.50 3 30 2022
    end
end

file open __stfx_probe_capture using `"`capture_file'"', write text replace
file write __stfx_probe_capture "STFIXEST_FE_INTERACTION_PROBE|schema|1" _n
file write __stfx_probe_capture "STFIXEST_FE_INTERACTION_PROBE|manifest_sha256|`manifest_sha256'" _n
file write __stfx_probe_capture "STFIXEST_FE_INTERACTION_PROBE|environment|stata_version|`c(stata_version)'" _n
file write __stfx_probe_capture "STFIXEST_FE_INTERACTION_PROBE|environment|stata_flavor|`c(flavor)'" _n
file write __stfx_probe_capture "STFIXEST_FE_INTERACTION_PROBE|environment|stata_processors|`c(processors)'" _n
file write __stfx_probe_capture "STFIXEST_FE_INTERACTION_PROBE|environment|stata_processors_limit|`c(processors_lic)'" _n
file write __stfx_probe_capture "STFIXEST_FE_INTERACTION_PROBE|environment|os|`c(os)'" _n
file write __stfx_probe_capture "STFIXEST_FE_INTERACTION_PROBE|environment|machine_type|`c(machine_type)'" _n

__stfx_probe_target
capture noisily reghdfe y c.x1, absorb(worker firm#year) residuals(__stfx_probe_resid)
local rc = _rc
__stfx_probe_emit target_default_firm_year `rc'

__stfx_probe_target
capture noisily reghdfe y c.x1, absorb(worker firm#year) keepsingletons residuals(__stfx_probe_resid)
local rc = _rc
__stfx_probe_emit target_keepsingletons `rc'

__stfx_probe_cascade
capture noisily reghdfe y c.x, absorb(worker firm#year) residuals(__stfx_probe_resid)
local rc = _rc
__stfx_probe_emit tuple_second_fe_cascade `rc'

__stfx_probe_cascade
capture noisily reghdfe y c.x [fw=fw], absorb(worker firm#year) residuals(__stfx_probe_resid)
local rc = _rc
__stfx_probe_emit fweight_frequency_cascade `rc'

__stfx_probe_missing
capture noisily reghdfe y c.x, absorb(worker firm#year) keepsingletons residuals(__stfx_probe_resid)
local rc = _rc
__stfx_probe_emit missing_interaction_components `rc'

__stfx_probe_target
capture noisily reghdfe y c.x1, absorb(worker firm#year firm#year) residuals(__stfx_probe_resid)
local rc = _rc
__stfx_probe_emit repeated_identical_interaction `rc'

__stfx_probe_target
capture noisily reghdfe y c.x1, absorb(worker year#firm) residuals(__stfx_probe_resid)
local rc = _rc
__stfx_probe_emit interaction_order_year_firm `rc'

__stfx_probe_target
capture noisily reghdfe y c.x1, absorb(worker firm#year) dof(none) residuals(__stfx_probe_resid)
local rc = _rc
__stfx_probe_emit dof_none `rc'

__stfx_probe_target
capture noisily reghdfe y c.x1, absorb(worker firm#year) dof(firstpair) residuals(__stfx_probe_resid)
local rc = _rc
__stfx_probe_emit dof_firstpair `rc'

__stfx_probe_target
capture noisily reghdfe y c.x1, absorb(worker firm#year) vce(cluster firm) residuals(__stfx_probe_resid)
local rc = _rc
__stfx_probe_emit nested_simple_cluster `rc'

__stfx_probe_target
capture noisily reghdfe y c.x1, absorb(worker firm#year) vce(cluster firm#year) residuals(__stfx_probe_resid)
local rc = _rc
__stfx_probe_emit identical_interaction_cluster `rc'

__stfx_probe_cascade
capture noisily reghdfe y c.x, absorb(worker firm#year) vce(cluster cluster_u#cluster_v) residuals(__stfx_probe_resid)
local rc = _rc
__stfx_probe_emit unrelated_interaction_cluster `rc'

file write __stfx_probe_capture "END_FIXTURE|reghdfe_fe_interaction_probe|rc=0" _n
file close __stfx_probe_capture
