*! StFixest developer-only prepared OLS eclass transaction 0.1.0 22aug2026
program define _stfixest_estimate_ols_test, eclass sortpreserve
    version 18.0
    local hidden_cmdline `"_stfixest_estimate_ols_test `0'"'

    syntax varlist(fv numeric min=1) [if] [in], ///
        [NOCONStant AWEIGHT(varname numeric) FWEIGHT(varname numeric) ///
         PWEIGHT(varname numeric) ABSorb(string asis) CLUster(string asis) ///
         VCE(string) DOF(string) KEEPsingletons RESIDual(name) ///
         INJECT(string)]

    local requested_weights = (`"`aweight'"' != "") + ///
        (`"`fweight'"' != "") + (`"`pweight'"' != "")
    if `requested_weights' > 1 {
        di as error "aweight(), fweight(), and pweight() are mutually exclusive"
        exit 198
    }
    if `"`residual'"' != "" confirm new variable `residual'

    local inject = lower(strtrim(`"`inject'"'))
    if `"`inject'"' != "" & !inlist(`"`inject'"', ///
        "posthold", "postplugin", "poststripe", "postereturn", ///
        "postscalars", "postdiagnostics", "precommit", ///
        "breakpostplugin") {
        di as error "unknown developer transaction injection stage: `inject'"
        exit 198
    }

    local vce_wire = lower(strtrim(`"`vce'"'))
    if `"`vce_wire'"' == "" {
        if strtrim(`"`cluster'"') != "" local vce_wire "cluster"
        else if `"`pweight'"' != "" local vce_wire "robust"
        else local vce_wire "iid"
    }
    if inlist(`"`vce_wire'"', "unadjusted", "ols") local vce_wire "iid"
    if `"`vce_wire'"' == "clustered" local vce_wire "cluster"
    if !inlist(`"`vce_wire'"', "iid", "robust", "cluster") {
        di as error "hidden prepared OLS supports vce(iid), vce(robust), or vce(cluster)"
        exit 198
    }
    if `"`vce_wire'"' == "cluster" & strtrim(`"`cluster'"') == "" {
        di as error "vce(cluster) requires cluster()"
        exit 198
    }
    if `"`vce_wire'"' != "cluster" & strtrim(`"`cluster'"') != "" {
        di as error "cluster() is valid only with clustered inference"
        exit 198
    }
    if `"`vce_wire'"' == "iid" & `"`pweight'"' != "" {
        di as error "pweights require robust or clustered inference"
        exit 198
    }

    local dof_requested = lower(strtrim(`"`dof'"'))
    local dof_wire `"`dof_requested'"'
    if `"`dof_wire'"' == "" local dof_wire "pairwise"
    if !inlist(`"`dof_wire'"', "none", "firstpair", "pairwise") {
        di as error "hidden prepared OLS supports dof(none), dof(firstpair), or dof(pairwise)"
        exit 198
    }

    marksample touse, novarlist
    gettoken depvar indepvars : varlist
    local absorb_option
    local cluster_option
    if strtrim(`"`absorb'"') != "" local absorb_option `"absorb(`absorb')"'
    if strtrim(`"`cluster'"') != "" local cluster_option `"cluster(`cluster')"'

    tempname model_sources model_terms model_components absorb_terms
    tempname absorb_components cluster_terms cluster_components input_bindings
    if strtrim(`"`indepvars'"') == "" {
        // `syntax [varlist]' expands an omitted varlist to `_all'; supply the
        // numeric marker solely to enter the parser, whose `emptymodel` branch
        // clears it before constructing the wire model.
        stfixest_parse `touse', touse(`touse') emptymodel ///
            `absorb_option' `cluster_option'
    }
    else {
        stfixest_parse `indepvars', touse(`touse') `absorb_option' `cluster_option'
    }

    local source_vars `"`r(source_vars)'"'
    local coefficient_names `"`r(coefficient_names)'"'
    local component_raw `"`r(component_raw_operators)'"'
    local component_ts `"`r(component_ts_operators)'"'
    local absorb_labels `"`r(absorb_labels)'"'
    local absorb_label_registry `"`r(absorb_label_registry)'"'
    local absorb_saved_registry `"`r(absorb_saved_name_registry)'"'
    local cluster_labels `"`r(cluster_labels)'"'
    local n_sources = r(n_sources)
    local n_terms = r(n_model_terms)
    local n_components = r(n_model_components)
    local n_absorb_terms = r(n_absorb_terms)
    local n_absorb_components = r(n_absorb_components)
    local n_cluster_terms = r(n_cluster_terms)
    local n_cluster_components = r(n_cluster_components)
    local model_major = r(model_wire_major)
    local model_minor = r(model_wire_minor)
    matrix `model_sources' = r(model_sources)
    matrix `model_terms' = r(model_terms)
    matrix `model_components' = r(model_components)
    matrix `absorb_terms' = r(absorb_terms)
    matrix `absorb_components' = r(absorb_components)
    matrix `cluster_terms' = r(cluster_terms)
    matrix `cluster_components' = r(cluster_components)

    foreach source of local source_vars {
        capture confirm numeric variable `source'
        if _rc {
            di as error "hidden prepared OLS supports only numeric sources"
            exit 498
        }
    }
    if `n_absorb_terms' > 0 {
        forvalues row = 1/`n_absorb_terms' {
            if `absorb_terms'[`row',2] != 1 | `absorb_terms'[`row',3] != 1 | ///
               `absorb_terms'[`row',4] != 0 {
                di as error "hidden prepared OLS supports only single-source intercept fixed effects"
                exit 498
            }
        }
    }

    local source_registry
    foreach value of local source_vars {
        local piece `"`=strlen(`"`value'"')':`value'"'
        if `"`source_registry'"' == "" local source_registry `"`piece'"'
        else local source_registry `"`source_registry'|`piece'"'
    }
    if `n_sources' == 0 local source_registry "."

    local token_registry
    foreach value of local coefficient_names {
        local piece `"`=strlen(`"`value'"')':`value'"'
        if `"`token_registry'"' == "" local token_registry `"`piece'"'
        else local token_registry `"`token_registry'|`piece'"'
    }
    if `n_terms' == 0 local token_registry "."

    local raw_registry
    foreach piece of local component_raw {
        if `"`raw_registry'"' == "" local raw_registry `"`piece'"'
        else local raw_registry `"`raw_registry'|`piece'"'
    }
    local ts_registry
    foreach piece of local component_ts {
        if `"`ts_registry'"' == "" local ts_registry `"`piece'"'
        else local ts_registry `"`ts_registry'|`piece'"'
    }
    if `n_components' == 0 {
        local raw_registry "."
        local ts_registry "."
    }

    local cluster_label_registry
    foreach value of local cluster_labels {
        local piece `"`=strlen(`"`value'"')':`value'"'
        if `"`cluster_label_registry'"' == "" local cluster_label_registry `"`piece'"'
        else local cluster_label_registry `"`cluster_label_registry'|`piece'"'
    }
    if `n_cluster_terms' == 0 local cluster_label_registry "."

    local has_aweight = (`"`aweight'"' != "")
    local has_fweight = (`"`fweight'"' != "")
    local has_pweight = (`"`pweight'"' != "")
    local has_weight = `has_aweight' + `has_fweight' + `has_pweight'
    local n_bindings = 2 + `n_sources' + `has_weight'
    matrix `input_bindings' = J(`n_bindings', 3, 0)
    matrix `input_bindings'[1,1] = 1
    matrix `input_bindings'[1,2] = 1
    matrix `input_bindings'[2,1] = 2
    matrix `input_bindings'[2,2] = 2
    if `n_sources' > 0 {
        forvalues source = 1/`n_sources' {
            local row = 2 + `source'
            local position = 2 + `source'
            matrix `input_bindings'[`row',1] = `position'
            matrix `input_bindings'[`row',2] = 3
            matrix `input_bindings'[`row',3] = `source'
        }
    }
    if `has_weight' {
        local row = 3 + `n_sources'
        local position = 3 + `n_sources'
        matrix `input_bindings'[`row',1] = `position'
        if `has_aweight' matrix `input_bindings'[`row',2] = 4
        if `has_fweight' matrix `input_bindings'[`row',2] = 5
        if `has_pweight' matrix `input_bindings'[`row',2] = 6
    }

    local include_intercept = (`"`noconstant'"' == "")
    local keep_singletons = (`"`keepsingletons'"' != "")
    local coefficient_count = `n_terms' + `include_intercept'
    local matrix_dimension = max(`coefficient_count', 1)
    local fe_rows = max(`n_absorb_terms', 1)
    local cluster_rows = max(`n_cluster_terms', 1)
    local subset_rows = max(2^`n_cluster_terms' - 1, 1)

    tempvar sample_shadow residual_shadow
    generate byte `sample_shadow' = .
    local output_sample = `n_bindings' + 1
    local output_residual 0
    local output_variables `sample_shadow'
    if `"`residual'"' != "" {
        generate double `residual_shadow' = .
        label variable `residual_shadow' "Residuals"
        local output_residual = `n_bindings' + 2
        local output_variables `output_variables' `residual_shadow'
    }

    tempname b_shadow v_shadow status_shadow fit_shadow dof_shadow
    tempname solver_shadow fe_shadow cluster_shadow subset_shadow
    tempname certificate_shadow receipt_shadow
    matrix `b_shadow' = J(1, `matrix_dimension', .)
    matrix `v_shadow' = J(`matrix_dimension', `matrix_dimension', .)
    matrix `status_shadow' = J(`matrix_dimension', 4, .)
    matrix `fit_shadow' = J(12, 2, .)
    matrix `dof_shadow' = J(20, 2, .)
    matrix `solver_shadow' = J(2, 12, .)
    matrix `fe_shadow' = J(`fe_rows', 8, .)
    matrix `cluster_shadow' = J(`cluster_rows', 4, .)
    matrix `subset_shadow' = J(`subset_rows', 4, .)
    matrix `certificate_shadow' = J(32, 2, .)
    matrix `receipt_shadow' = J(1, 16, 0)

    tempname held_estimates
    tempvar held_sample
    _estimates hold `held_estimates', restore nullok varname(`held_sample')
    if `"`inject'"' == "posthold" exit 3498

    local weight_variable `"`aweight'`fweight'`pweight'"'
    local account_cluster_nesting = (`"`vce_wire'"' == "cluster")
    local vce_code = cond(`"`vce_wire'"' == "iid", 1, ///
        cond(`"`vce_wire'"' == "robust", 2, 3))

    _stfixest_load, action(loadonly)
    local plugin_file `"`r(plugin_file)'"'
    capture program __stfixest_estimate_plugin, plugin using(`"`plugin_file'"')
    local load_rc = _rc
    if `load_rc' != 0 & `load_rc' != 110 exit `load_rc'
    local request_arguments `"ado=0.1.0-phase01;protocol_major=1;protocol_minor=1;stata=`c(stata_version)';confirm=developer-estimation-test;plugin=0.1.0;result_major=1;result_minor=0;model_major=`model_major';model_minor=`model_minor';source_matrix=`model_sources';source_count=`n_sources';term_matrix=`model_terms';term_count=`n_terms';component_matrix=`model_components';component_count=`n_components';source_names=`source_registry';canonical_tokens=`token_registry';raw_component_operators=`raw_registry';raw_time_series_operators=`ts_registry';absorb_term_matrix=`absorb_terms';absorb_term_count=`n_absorb_terms';absorb_component_matrix=`absorb_components';absorb_component_count=`n_absorb_components';absorb_labels=`absorb_label_registry';absorb_saved_names=`absorb_saved_registry';cluster_term_matrix=`cluster_terms';cluster_term_count=`n_cluster_terms';cluster_component_matrix=`cluster_components';cluster_component_count=`n_cluster_components';cluster_labels=`cluster_label_registry';binding_matrix=`input_bindings';binding_count=`n_bindings';include_intercept=`include_intercept';keep_singletons=`keep_singletons';vce=`vce_wire';dof=`dof_wire';account_cluster_nesting=`account_cluster_nesting';output_sample=`output_sample';output_residual=`output_residual';output_drop_reason=0;output_b_matrix=`b_shadow';output_v_matrix=`v_shadow';output_status_matrix=`status_shadow';output_fit_matrix=`fit_shadow';output_dof_matrix=`dof_shadow';output_solver_matrix=`solver_shadow';output_fe_matrix=`fe_shadow';output_cluster_matrix=`cluster_shadow';output_subset_matrix=`subset_shadow';output_certificate_matrix=`certificate_shadow';output_receipt_matrix=`receipt_shadow';"'
    local request_bundle
    local request_count 0
    while `"`request_arguments'"' != "" {
        gettoken argument request_arguments : request_arguments, parse(";")
        if `"`argument'"' == ";" continue
        local argument = strtrim(`"`argument'"')
        local ++request_count
        local argument_length = strlen(`"`argument'"')
        local request_bundle `"`request_bundle'`argument_length':`argument'"'
    }
    local request_bundle `"`request_count':`request_bundle'"'
    plugin call __stfixest_estimate_plugin `touse' `depvar' `source_vars' ///
        `weight_variable' `output_variables', ///
        `"estimate-ols-test"' `"bundle=`request_bundle'"'

    if `"`inject'"' == "breakpostplugin" {
        di as result "STFIXEST_BREAK_ARMED"
        sleep 30000
        exit 3498
    }
    if `"`inject'"' == "postplugin" exit 3498

    _stfixest_eols_validate `b_shadow' `v_shadow' ///
        `status_shadow' `fit_shadow' `dof_shadow' `solver_shadow' ///
        `fe_shadow' `cluster_shadow' `subset_shadow' ///
        `certificate_shadow' `receipt_shadow', ///
        coefficients(`coefficient_count') vcecode(`vce_code') ///
        absorbrows(`fe_rows') clusters(`n_cluster_terms') ///
        subsets(`subset_rows') residual(`=(`"`residual'"' != "")')

    local posted_names `"`coefficient_names'"'
    if `include_intercept' local posted_names `"`posted_names' _cons"'
    local posted_name_count : word count `posted_names'
    if `posted_name_count' != `coefficient_count' {
        di as error "hidden OLS coefficient-name count disagrees with receipt"
        exit 459
    }

    if `coefficient_count' > 0 {
        matrix colnames `b_shadow' = `posted_names'
        matrix colnames `v_shadow' = `posted_names'
        matrix rownames `v_shadow' = `posted_names'
        tempname marker_b marker_v posted_b posted_v
        matrix `marker_b' = J(1, `coefficient_count', 0)
        matrix `marker_v' = J(`coefficient_count', `coefficient_count', 0)
        forvalues column = 1/`coefficient_count' {
            local status = `status_shadow'[`column',2]
            if inlist(`status', 1, 5) {
                matrix `marker_b'[1,`column'] = 1
                matrix `marker_v'[`column',`column'] = 1
            }
        }
        matrix colnames `marker_b' = `posted_names'
        matrix colnames `marker_v' = `posted_names'
        matrix rownames `marker_v' = `posted_names'
        _ms_findomitted `marker_b' `marker_v'
        local marked_names : colfullnames `marker_b'
        matrix colnames `b_shadow' = `marked_names'
        matrix colnames `v_shadow' = `marked_names'
        matrix rownames `v_shadow' = `marked_names'
        mata: st_matrix("`posted_b'", st_matrix("`b_shadow'"))
        mata: st_matrix("`posted_v'", st_matrix("`v_shadow'"))
        matrix colnames `posted_b' = `marked_names'
        matrix colnames `posted_v' = `marked_names'
        matrix rownames `posted_v' = `marked_names'
    }
    if `"`inject'"' == "poststripe" exit 3498

    local N_effective = `dof_shadow'[1,2]
    local df_m = `dof_shadow'[7,2]
    local df_r = `dof_shadow'[10,2]
    local eweight
    if `has_aweight' local eweight "[aweight=`aweight']"
    if `has_fweight' local eweight "[fweight=`fweight']"
    if `has_pweight' local eweight "[pweight=`pweight']"
    if `coefficient_count' > 0 {
        // Whole-matrix Mata construction preserves the pinned multiway CGM
        // matrix through Stata's e(V) posting path. Cell-written SPI shadows
        // are never posted directly, and the protected marker above is the
        // only input to `_ms_findomitted`.
        ereturn post `posted_b' `posted_v' `eweight', ///
            depname(`depvar') obs(`N_effective') dof(`df_r') ///
            esample(`sample_shadow') buildfvinfo
    }
    else {
        ereturn post `eweight', depname(`depvar') obs(`N_effective') ///
            dof(`df_r') esample(`sample_shadow') buildfvinfo
    }
    if `"`inject'"' == "postereturn" exit 3498

    local fit_names rss tss tss_within mss r2 r2_a r2_within r2_a_within rmse F ll ll_0
    forvalues row = 1/12 {
        local fit_name : word `row' of `fit_names'
        if `fit_shadow'[`row',1] == 1 {
            ereturn scalar `fit_name' = `fit_shadow'[`row',2]
        }
        else ereturn scalar `fit_name' = .
    }
    ereturn scalar rank = `df_m'
    ereturn scalar df_m = `df_m'
    ereturn scalar df_a = `dof_shadow'[8,2]
    ereturn scalar df_a_initial = `dof_shadow'[14,2]
    ereturn scalar df_a_redundant = `dof_shadow'[15,2]
    ereturn scalar df_a_nested = `dof_shadow'[13,2]
    ereturn scalar num_singletons = `dof_shadow'[16,2]
    ereturn scalar drop_singletons = 1 - `keep_singletons'
    ereturn scalar N_full = `N_effective' + `dof_shadow'[16,2]
    if `dof_shadow'[5,1] == 1 ereturn scalar sumweights = `dof_shadow'[5,2]
    else ereturn scalar sumweights = `N_effective'
    ereturn scalar ic = `solver_shadow'[1,3]
    ereturn scalar converged = 1
    ereturn scalar report_constant = `include_intercept'
    ereturn scalar N_hdfe = `fe_rows'
    ereturn scalar N_hdfe_extended = `fe_rows'
    ereturn scalar stfixest_df_fit = `dof_shadow'[11,2]
    ereturn scalar stfixest_df_correction = `dof_shadow'[12,2]
    if `"`inject'"' == "postscalars" exit 3498

    tempname posted_dof_table
    matrix `posted_dof_table' = J(`fe_rows', 5, 0)
    local dof_row_names
    forvalues row = 1/`fe_rows' {
        matrix `posted_dof_table'[`row',1] = `fe_shadow'[`row',4]
        matrix `posted_dof_table'[`row',2] = `fe_shadow'[`row',5]
        matrix `posted_dof_table'[`row',3] = `fe_shadow'[`row',6]
        matrix `posted_dof_table'[`row',4] = 1 - `fe_shadow'[`row',7]
        matrix `posted_dof_table'[`row',5] = `fe_shadow'[`row',8]
        if `fe_shadow'[`row',1] == 2 local dof_row_name "_cons"
        else {
            local term = `fe_shadow'[`row',2]
            local dof_row_name : word `term' of `absorb_labels'
            local dof_row_name `"1.`dof_row_name'"'
        }
        local dof_row_names `"`dof_row_names' `dof_row_name'"'
    }
    matrix rownames `posted_dof_table' = `dof_row_names'
    mata: st_matrixcolstripe("`posted_dof_table'", (J(5,1,""), ("Categories" \ "Redundant" \ "Num Coefs" \ "Inexact?" \ "Nested?")))
    ereturn matrix dof_table = `posted_dof_table', copy
    ereturn matrix stfixest_status = `status_shadow', copy
    ereturn matrix stfixest_fit = `fit_shadow', copy
    ereturn matrix stfixest_dof = `dof_shadow', copy
    ereturn matrix stfixest_solver = `solver_shadow', copy
    ereturn matrix stfixest_fe = `fe_shadow', copy
    ereturn matrix stfixest_cluster = `cluster_shadow', copy
    ereturn matrix stfixest_subset = `subset_shadow', copy
    ereturn matrix stfixest_certificate = `certificate_shadow', copy
    ereturn matrix stfixest_receipt = `receipt_shadow', copy

    local kept_indepvars
    if `n_terms' > 0 {
        forvalues row = 1/`n_terms' {
            if `status_shadow'[`row',2] == 1 {
                local name : word `row' of `coefficient_names'
                local kept_indepvars `"`kept_indepvars' `name'"'
            }
        }
    }
    local kept_indepvars = strtrim(`"`kept_indepvars'"')
    ereturn local cmd "_stfixest_estimate_ols_test"
    ereturn local cmdline `"`hidden_cmdline'"'
    ereturn local model "ols"
    ereturn local depvar "`depvar'"
    ereturn local indepvars `"`kept_indepvars'"'
    local posted_vce = cond(`"`vce_wire'"' == "iid", "ols", `"`vce_wire'"')
    ereturn local vce "`posted_vce'"
    if `"`vce_wire'"' != "iid" ereturn local vcetype "Robust"
    local posted_absvars = cond(strtrim(`"`absorb_labels'"') == "", "_cons", strtrim(`"`absorb_labels'"'))
    ereturn local absvars `"`posted_absvars'"'
    ereturn local extended_absvars `"`posted_absvars'"'
    if `"`dof_requested'"' == "" ereturn local dofmethod "pairwise clusters continuous"
    else if `"`dof_wire'"' != "none" ereturn local dofmethod "`dof_wire'"
    ereturn local title "HDFE Linear regression"
    ereturn local title2 "Absorbing `fe_rows' HDFE group"
    if `"`vce_wire'"' == "cluster" {
        local posted_cluster_labels = strtrim(`"`cluster_labels'"')
        local minimum_clusters .
        forvalues row = 1/`n_cluster_terms' {
            local groups = `cluster_shadow'[`row',3]
            if missing(`minimum_clusters') | `groups' < `minimum_clusters' local minimum_clusters = `groups'
            local cluster_label : word `row' of `cluster_labels'
            ereturn scalar N_clust`row' = `groups'
            ereturn local clustvar`row' `"`cluster_label'"'
        }
        ereturn scalar N_clust = `minimum_clusters'
        ereturn scalar N_clustervars = `n_cluster_terms'
        ereturn local clustvar `"`posted_cluster_labels'"'
        ereturn local title3 "Statistics robust to heteroskedasticity"
    }
    if `has_aweight' {
        ereturn local wtype "aweight"
        ereturn local wexp "= `aweight'"
    }
    if `has_fweight' {
        ereturn local wtype "fweight"
        ereturn local wexp "= `fweight'"
    }
    if `has_pweight' {
        ereturn local wtype "pweight"
        ereturn local wexp "= `pweight'"
    }
    if `"`residual'"' != "" ereturn local resid "`residual'"
    ereturn local stfixest_action_protocol "1.1"
    ereturn local stfixest_model_wire "2.1"
    ereturn local stfixest_result_wire "1.0"
    ereturn local stfixest_plugin "0.1.0"
    if `"`inject'"' == "postdiagnostics" exit 3498
    if `"`inject'"' == "precommit" exit 3498

    nobreak {
        if `"`residual'"' != "" rename `residual_shadow' `residual'
        _estimates unhold `held_estimates', not
    }
end


program define _stfixest_eols_validate
    version 18.0
    syntax namelist(min=11 max=11), COEFFICIENTS(integer) VCECODE(integer) ABSORBROWS(integer) ///
        CLUSTERS(integer) SUBSETS(integer) RESIDUAL(integer)
    local b : word 1 of `namelist'
    local V : word 2 of `namelist'
    local status : word 3 of `namelist'
    local fit : word 4 of `namelist'
    local dof : word 5 of `namelist'
    local solver : word 6 of `namelist'
    local fe : word 7 of `namelist'
    local clusters_table : word 8 of `namelist'
    local subsets_table : word 9 of `namelist'
    local certificates : word 10 of `namelist'
    local receipt : word 11 of `namelist'

    local physical = max(`coefficients', 1)
    local expected_subsets = cond(`clusters' == 0, 0, `subsets')
    foreach column in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 {
        if missing(`receipt'[1,`column']) {
            di as error "hidden OLS receipt contains missing data"
            exit 459
        }
    }
    if `receipt'[1,1] != 5354601 | `receipt'[1,2] != 1 | ///
       `receipt'[1,3] != 0 | `receipt'[1,4] != `vcecode' | ///
       `receipt'[1,5] != `coefficients' | ///
       `receipt'[1,7] != _N | `receipt'[1,10] != `clusters' | ///
       `receipt'[1,11] != `absorbrows' | ///
       `receipt'[1,12] != `expected_subsets' | ///
       `receipt'[1,13] != 1 | `receipt'[1,14] != `residual' | ///
       `receipt'[1,15] != 1 | `receipt'[1,16] != 1 {
        di as error "hidden OLS receipt failed exact structural validation"
        exit 459
    }
    if `receipt'[1,6] < 0 | `receipt'[1,6] > `coefficients' | ///
       `receipt'[1,8] < 0 | `receipt'[1,8] > _N | ///
       `receipt'[1,9] < 0 | `receipt'[1,9] > `receipt'[1,8] {
        di as error "hidden OLS receipt count is out of range"
        exit 459
    }

    tempname coordinate_seen
    matrix `coordinate_seen' = J(1, max(`receipt'[1,6], 1), 0)
    local covariance_count = 0
    forvalues row = 1/`physical' {
        local slot = `status'[`row',1]
        local code = `status'[`row',2]
        local coordinate = `status'[`row',4]
        if `coefficients' == 0 {
            if `slot' != 0 | `code' != 0 | `status'[`row',3] != 0 | `coordinate' != 0 | ///
               `b'[1,`row'] != 0 | `V'[`row',`row'] != 0 {
                di as error "hidden OLS K0 sentinel is not zero"
                exit 459
            }
        }
        else {
            if `slot' != `row' | !inlist(`code', 1, 2, 3, 4, 5) {
                di as error "hidden OLS coefficient status is invalid"
                exit 459
            }
            if missing(`status'[`row',3]) | `status'[`row',3] < 0 | ///
               `status'[`row',3] != floor(`status'[`row',3]) {
                di as error "hidden OLS generated-column status is invalid"
                exit 459
            }
            if inlist(`code', 1, 5) {
                local ++covariance_count
                if `coordinate' <= 0 | `coordinate' > `receipt'[1,6] | missing(`b'[1,`row']) {
                    di as error "hidden OLS kept coordinate is invalid"
                    exit 459
                }
                if `coordinate_seen'[1,`coordinate'] != 0 {
                    di as error "hidden OLS covariance coordinate is duplicated"
                    exit 459
                }
                matrix `coordinate_seen'[1,`coordinate'] = 1
            }
            else {
                if `coordinate' != 0 | `b'[1,`row'] != 0 {
                    di as error "hidden OLS omitted coordinate was not scattered to zero"
                    exit 459
                }
                forvalues column = 1/`physical' {
                    if `V'[`row',`column'] != 0 | `V'[`column',`row'] != 0 {
                        di as error "hidden OLS omitted covariance row was not scattered to zero"
                        exit 459
                    }
                }
            }
        }
    }
    if `covariance_count' != `receipt'[1,6] {
        di as error "hidden OLS covariance dimension disagrees with status table"
        exit 459
    }
    if `coefficients' > 0 {
        forvalues row = 1/`physical' {
            forvalues column = 1/`physical' {
                if missing(`V'[`row',`column']) | ///
                   abs(`V'[`row',`column'] - `V'[`column',`row']) > 1e-10 {
                    di as error "hidden OLS covariance is missing or asymmetric"
                    exit 459
                }
            }
            if `V'[`row',`row'] < 0 {
                di as error "hidden OLS covariance has a negative diagonal"
                exit 459
            }
        }
    }
    foreach table in `fit' `dof' `certificates' {
        local rows = rowsof(`table')
        forvalues row = 1/`rows' {
            if !inlist(`table'[`row',1], 0, 1) | ///
               (`table'[`row',1] == 0 & `table'[`row',2] != 0) | ///
               missing(`table'[`row',2]) {
                di as error "hidden OLS defined/value table is invalid"
                exit 459
            }
        }
    }
    foreach row in 1 2 3 24 32 {
        if `certificates'[`row',1] != 1 | `certificates'[`row',2] != 1 {
            di as error "hidden OLS certificate did not pass"
            exit 459
        }
    }
    if `certificates'[31,1] != 1 | missing(`certificates'[31,2]) {
        di as error "hidden OLS covariance determinant metadata is invalid"
        exit 459
    }
    forvalues row = 1/2 {
        forvalues column = 1/12 {
            if missing(`solver'[`row',`column']) {
                di as error "hidden OLS solver metadata is nonfinite"
                exit 459
            }
        }
        if !inlist(`solver'[`row',1], 1, 2, 3) | ///
           !inlist(`solver'[`row',2], 1, 2, 3) | ///
           `solver'[`row',3] < 0 | ///
           !inlist(`solver'[`row',4], 1, 2, 3, 4, 5) | ///
           !inlist(`solver'[`row',11], 0, 1) | `solver'[`row',12] < 0 {
            di as error "hidden OLS solver metadata is invalid"
            exit 459
        }
    }
    if `dof'[1,1] != 1 | `dof'[7,1] != 1 | `dof'[8,1] != 1 | ///
       `dof'[10,1] != 1 | `dof'[1,2] <= 0 | `dof'[10,2] <= 0 {
        di as error "hidden OLS required DoF fields are invalid"
        exit 459
    }
    forvalues row = 1/`absorbrows' {
        if !inlist(`fe'[`row',1], 1, 2) | `fe'[`row',4] < 1 | ///
           `fe'[`row',5] < 0 | `fe'[`row',6] < 0 | ///
           !inlist(`fe'[`row',7], 0, 1) | !inlist(`fe'[`row',8], 0, 1) {
            di as error "hidden OLS FE metadata identity is invalid"
            exit 459
        }
    }
    if `clusters' == 0 {
        forvalues column = 1/4 {
            if `clusters_table'[1,`column'] != 0 | `subsets_table'[1,`column'] != 0 {
                di as error "hidden nonclustered OLS has nonempty cluster sentinels"
                exit 459
            }
        }
    }
    else {
        forvalues row = 1/`clusters' {
            if `clusters_table'[`row',1] != `row' | `clusters_table'[`row',3] < 2 {
                di as error "hidden OLS cluster metadata is invalid"
                exit 459
            }
        }
        forvalues row = 1/`subsets' {
            if `subsets_table'[`row',1] < 1 | ///
               `subsets_table'[`row',1] > 2^`clusters' - 1 | ///
               `subsets_table'[`row',2] < 1 | ///
               `subsets_table'[`row',2] > `clusters' | ///
               !inlist(`subsets_table'[`row',3], -1, 1) | ///
               `subsets_table'[`row',4] < 1 {
                di as error "hidden OLS cluster subset metadata is invalid"
                exit 459
            }
        }
    }
end
