*! fereg guarded prepared OLS eclass transaction 0.5.0 24aug2026
// This path preserves observation order directly: factor parsing and TS
// evaluation do not sort, and native row maps are used for all output stores.
// Do not add a sorting operation without restoring order on every exit.
program define _fereg_estimate_ols, eclass
    version 18.0
    quietly _fereg_memory_baseline
    local stata_used_entry = strtrim(string(r(used), "%21.0f"))
    local stata_alloc_entry = strtrim(string(r(allocated), "%21.0f"))
    local hidden_cmdline `"_fereg_estimate_ols `0'"'

    syntax varlist(fv ts numeric min=1) [if] [in], ///
        [NOCONStant AWEIGHT(varname numeric) FWEIGHT(varname numeric) ///
         PWEIGHT(varname numeric) ABSorb(string asis) CLUster(string asis) ///
         VCE(string) DOF(string) KEEPsingletons RESIDual(name) ///
         XB(name) XBD(name) D(name) ///
         SOLVer(string) PREConditioner(string) ///
         MEMory(string) POOLSize(integer 10) TOLerance(real 1e-8) ///
         ITERate(integer 16000) THREADS(integer 0) DETERMINISTIC ///
         INJECT(string)]

    local memory_mode = lower(strtrim(`"`memory'"'))
    if `"`memory_mode'"' == "" local memory_mode "fast"
    if !inlist(`"`memory_mode'"', "auto", "fast", "low") | ///
       `poolsize' <= 0 | `threads' < 0 | `tolerance' <= 0 | ///
       `tolerance' >= 1 | `iterate' <= 0 {
        di as error "memory(), poolsize(), tolerance(), or iterate() is invalid"
        exit 198
    }

    local solver_mode = lower(strtrim(`"`solver'"'))
    if `"`solver_mode'"' == "" local solver_mode "auto"
    if !inlist(`"`solver_mode'"', "auto", "map", "lsmr", "cmg", "cg", "it", "squarem") {
        di as error "solver() must be auto, map, lsmr, cmg, cg, it, or squarem"
        exit 198
    }
    local preconditioner_mode = lower(strtrim(`"`preconditioner'"'))
    if `"`preconditioner_mode'"' == "" {
        if !inlist(`"`solver_mode'"', "auto", "lsmr") local preconditioner_mode "off"
        else local preconditioner_mode "auto"
    }
    if `"`preconditioner_mode'"' == "none" local preconditioner_mode "off"
    if `"`preconditioner_mode'"' == "additive" local preconditioner_mode "schwarz"
    if !inlist(`"`preconditioner_mode'"', "auto", "off", "diagonal", "schwarz") {
        di as error "preconditioner() must be auto, off, diagonal, or schwarz"
        exit 198
    }
    if !inlist(`"`solver_mode'"', "auto", "lsmr") & `"`preconditioner_mode'"' != "off" {
        di as error "preconditioner() is valid only with solver(auto) or solver(lsmr)"
        exit 198
    }

    local requested_weights = (`"`aweight'"' != "") + ///
        (`"`fweight'"' != "") + (`"`pweight'"' != "")
    if `requested_weights' > 1 {
        di as error "aweight(), fweight(), and pweight() are mutually exclusive"
        exit 198
    }
    if `"`residual'"' != "" confirm new variable `residual'
    foreach output in `xb' `xbd' `d' {
        if `"`output'"' != "" confirm new variable `output'
    }

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
    if !inlist(`"`dof_wire'"', "none", "firstpair", "pairwise", "continuous") {
        di as error "hidden prepared OLS supports dof(none), dof(firstpair), dof(pairwise), or dof(continuous)"
        exit 198
    }

    marksample touse, novarlist
    local candidate_all = (strtrim(`"`if'"') == "" & strtrim(`"`in'"') == "")
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
        fereg_parse `touse', touse(`touse') emptymodel ///
            `absorb_option' `cluster_option'
    }
    else {
        fereg_parse `indepvars', touse(`touse') `absorb_option' `cluster_option'
    }

    local source_vars `"`r(source_vars)'"'
    local coefficient_names `"`r(coefficient_names)'"'
    local component_raw `"`r(component_raw_operators)'"'
    local component_ts `"`r(component_ts_operators)'"'
    local absorb_labels `"`r(absorb_labels)'"'
    local absorb_saved_names `"`r(absorb_saved_names)'"'
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

    /* Stata owns time-series indexing and gap semantics. Materialize only the
       pending TS components, then bind each generated numeric source explicitly
       while retaining the original coefficient token and raw TS metadata. */
    local original_source_vars `"`source_vars'"'
    tempname original_model_components
    matrix `original_model_components' = `model_components'
    if `n_components' > 0 {
        forvalues component_row = 1/`n_components' {
            if `original_model_components'[`component_row',9] != 2 continue

            local source_id = `original_model_components'[`component_row',3]
            local base_source : word `source_id' of `original_source_vars'
            local ts_piece : word `component_row' of `component_ts'
            local ts_colon = strpos(`"`ts_piece'"', ":")
            if `ts_colon' == 0 {
                di as error "malformed time-series operator registry"
                exit 198
            }
            local ts_operator = substr(`"`ts_piece'"', `ts_colon' + 1, .)
            if strtrim(`"`ts_operator'"') == "" {
                di as error "pending time-series component has no operator"
                exit 198
            }
            local ts_expression `"`ts_operator'"'
            if substr(`"`ts_expression'"', strlen(`"`ts_expression'"'), 1) != "." ///
                local ts_expression `"`ts_expression'."'

            tempvar materialized_ts
            capture quietly generate double `materialized_ts' = `ts_expression'`base_source'
            if _rc {
                local materialize_rc = _rc
                di as error "unable to materialize time-series source `ts_expression'`base_source'"
                exit `materialize_rc'
            }

            local source_has_non_ts_use 0
            forvalues check = 1/`n_components' {
                if `original_model_components'[`check',3] == `source_id' & ///
                   `original_model_components'[`check',9] != 2 ///
                    local source_has_non_ts_use 1
            }
            if `n_absorb_components' > 0 {
                forvalues check = 1/`n_absorb_components' {
                    if `absorb_components'[`check',4] == `source_id' ///
                        local source_has_non_ts_use 1
                }
            }
            if `n_cluster_components' > 0 {
                forvalues check = 1/`n_cluster_components' {
                    if `cluster_components'[`check',3] == `source_id' ///
                        local source_has_non_ts_use 1
                }
            }

            local source_already_reused 0
            if `component_row' > 1 {
                forvalues prior = 1/`=`component_row' - 1' {
                    if `original_model_components'[`prior',3] == `source_id' & ///
                       `original_model_components'[`prior',9] == 2 ///
                        local source_already_reused 1
                }
            }

            if !`source_has_non_ts_use' & !`source_already_reused' {
                local rebuilt_source_vars
                forvalues source_index = 1/`n_sources' {
                    local existing_source : word `source_index' of `source_vars'
                    if `source_index' == `source_id' ///
                        local rebuilt_source_vars `"`rebuilt_source_vars' `materialized_ts'"'
                    else local rebuilt_source_vars `"`rebuilt_source_vars' `existing_source'"'
                }
                local source_vars `"`rebuilt_source_vars'"'
                local materialized_source_id = `source_id'
            }
            else {
                local ++n_sources
                local source_vars `"`source_vars' `materialized_ts'"'
                matrix `model_sources' = `model_sources' \ (`n_sources', 0)
                local materialized_source_id = `n_sources'
            }
            matrix `model_components'[`component_row',3] = `materialized_source_id'
            matrix `model_components'[`component_row',9] = 3
        }
    }

    local has_absorb_interaction 0
    local n_absorb_intercepts 0
    local n_absorb_slopes 0
    if `n_absorb_terms' > 0 {
        forvalues row = 1/`n_absorb_terms' {
            if `absorb_terms'[`row',2] < 1 | ///
               !inlist(`absorb_terms'[`row',3], 0, 1) | ///
               (`absorb_terms'[`row',3] == 0 & `absorb_terms'[`row',4] < 1) {
                di as error "hidden prepared OLS absorbed terms require categorical sources and at least one intercept or slope channel"
                exit 498
            }
            local n_absorb_intercepts = `n_absorb_intercepts' + `absorb_terms'[`row',3]
            local n_absorb_slopes = `n_absorb_slopes' + `absorb_terms'[`row',4]
            if `absorb_terms'[`row',2] > 1 | `absorb_terms'[`row',4] > 0 ///
                local has_absorb_interaction 1
        }
    }
    if `has_absorb_interaction' {
        if regexm(lower(`"`indepvars'"'), "(^|[ #])ib[^ #]*[.]") {
            di as error "hidden categorical OLS final-sample rebasing does not yet support explicit base operators"
            exit 498
        }
        if `n_components' > 0 {
            forvalues row = 1/`n_components' {
                if `model_components'[`row',4] == 2 {
                    local source_id = `model_components'[`row',3]
                    local source : word `source_id' of `source_vars'
                    local fv_base : char `source'[_fv_base]
                    if `"`fv_base'"' != "" {
                        di as error "hidden categorical OLS final-sample rebasing does not yet support fvset base overrides"
                        exit 498
                    }
                }
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

    // Pinned reghdfe does not synthesize a structural constant when every
    // absorbed term is slope-only.  In that case `_cons` is absent even
    // without an explicit noconstant option.
    local include_intercept = (`"`noconstant'"' == "") & ///
        (`n_absorb_terms' == 0 | `n_absorb_intercepts' > 0)
    local keep_singletons = (`"`keepsingletons'"' != "")
    local coefficient_capacity = `n_terms' + `include_intercept'
    local matrix_dimension = max(`coefficient_capacity', 1)
    local fe_rows = max(`n_absorb_intercepts' + `n_absorb_slopes', 1)
    local cluster_rows = max(`n_cluster_terms', 1)
    local subset_rows = max(2^`n_cluster_terms' - 1, 1)

    tempvar residual_shadow xb_shadow xbd_shadow d_shadow
    // With an unrestricted candidate universe, Rust never reads the candidate
    // slot. Reuse the private marker as the result sample and bind that unused
    // input slot to the numeric outcome; scientific inputs remain read-only.
    local candidate_variable `touse'
    if `candidate_all' {
        local sample_shadow `touse'
        local candidate_variable `depvar'
    }
    else {
        tempvar sample_shadow
        generate byte `sample_shadow' = `touse'
    }
    local output_sample = `n_bindings' + 1
    local output_residual 0
    local output_xb 0
    local output_xbd 0
    local output_absorbed 0
    local output_variables `sample_shadow'
    if `"`residual'"' != "" {
        generate double `residual_shadow' = .
        label variable `residual_shadow' "Residuals"
        local output_residual = `n_bindings' + 2
        local output_variables `output_variables' `residual_shadow'
    }
    if `"`xb'"' != "" {
        generate double `xb_shadow' = .
        label variable `xb_shadow' "Linear prediction"
        local output_variables `output_variables' `xb_shadow'
        local output_variable_count : word count `output_variables'
        local output_xb = `n_bindings' + `output_variable_count'
    }
    if `"`xbd'"' != "" {
        generate double `xbd_shadow' = .
        label variable `xbd_shadow' "Linear prediction plus absorbed contribution"
        local output_variables `output_variables' `xbd_shadow'
        local output_variable_count : word count `output_variables'
        local output_xbd = `n_bindings' + `output_variable_count'
    }
    if `"`d'"' != "" {
        generate double `d_shadow' = .
        label variable `d_shadow' "Absorbed contribution"
        local output_variables `output_variables' `d_shadow'
        local output_variable_count : word count `output_variables'
        local output_absorbed = `n_bindings' + `output_variable_count'
    }

    local output_saved_count = `n_absorb_intercepts' + `n_absorb_slopes'
    local output_saved_registry
    local saved_shadow_variables
    local saved_target_variables
    if `n_absorb_terms' > 0 {
        forvalues term = 1/`n_absorb_terms' {
            local saved_base : word `term' of `absorb_saved_names'
            local intercept = `absorb_terms'[`term',3]
            local slopes = `absorb_terms'[`term',4]
            local channels = `intercept' + `slopes'
            if `channels' == 0 continue
            forvalues channel = 1/`channels' {
                local saved_position 0
                if `"`saved_base'"' != "." {
                    local target `"`saved_base'"'
                    if !`intercept' | `channel' > 1 {
                        local slope_index = `channel' - `intercept'
                        local target `"`saved_base'_Slope`slope_index'"'
                    }
                    tempvar saved_shadow
                    generate double `saved_shadow' = .
                    label variable `saved_shadow' "[FE] `target'"
                    local output_variables `output_variables' `saved_shadow'
                    local output_variable_count : word count `output_variables'
                    local saved_position = `n_bindings' + `output_variable_count'
                    local saved_shadow_variables `saved_shadow_variables' `saved_shadow'
                    local saved_target_variables `saved_target_variables' `target'
                }
                local saved_piece `"`=strlen("`saved_position'")':`saved_position'"'
                if `"`output_saved_registry'"' == "" ///
                    local output_saved_registry `"`saved_piece'"'
                else local output_saved_registry `"`output_saved_registry'|`saved_piece'"'
            }
        }
    }
    if `output_saved_count' == 0 local output_saved_registry "."

    local requested_output_names `residual' `xb' `xbd' `d' `saved_target_variables'
    local requested_output_names = strtrim(`"`requested_output_names'"')
    local unique_output_names : list uniq requested_output_names
    local requested_output_count : word count `requested_output_names'
    local unique_output_count : word count `unique_output_names'
    if `requested_output_count' != `unique_output_count' {
        di as error "OLS output variable names must be distinct"
        exit 110
    }

    tempname b_shadow v_shadow status_shadow marker_b_shadow marker_v_shadow
    tempname fit_shadow dof_shadow
    tempname solver_shadow fe_shadow cluster_shadow subset_shadow
    tempname memory_shadow
    matrix `memory_shadow' = J(11, 2, .)
    tempname certificate_shadow runtime_shadow receipt_shadow
    matrix `b_shadow' = J(1, `matrix_dimension', .)
    matrix `v_shadow' = J(`matrix_dimension', `matrix_dimension', .)
    matrix `status_shadow' = J(`matrix_dimension', 4, .)
    matrix `marker_b_shadow' = J(1, `matrix_dimension', .)
    matrix `marker_v_shadow' = J(`matrix_dimension', `matrix_dimension', .)
    matrix `fit_shadow' = J(12, 2, .)
    matrix `dof_shadow' = J(20, 2, .)
    matrix `solver_shadow' = J(2, 16, .)
    matrix `fe_shadow' = J(`fe_rows', 8, .)
    matrix `cluster_shadow' = J(`cluster_rows', 4, .)
    matrix `subset_shadow' = J(`subset_rows', 4, .)
    matrix `certificate_shadow' = J(32, 2, .)
    matrix `runtime_shadow' = J(30, 2, .)
    matrix `receipt_shadow' = J(1, 16, 0)

    tempname held_estimates
    tempvar held_sample
    _estimates hold `held_estimates', restore nullok varname(`held_sample')
    if `"`inject'"' == "posthold" exit 3498

    local weight_variable `"`aweight'`fweight'`pweight'"'
    // Rust loads numeric values and detects missingness in the same guarded
    // call; a separate full-data markout certificate is unnecessary.
    local input_complete 0
    local account_cluster_nesting = (`"`vce_wire'"' == "cluster")
    local vce_code = cond(`"`vce_wire'"' == "iid", 1, ///
        cond(`"`vce_wire'"' == "robust", 2, 3))

    // The plugin program persists for the Stata process. Warm estimates avoid
    // repeating ado-path discovery and a knowingly failing duplicate program
    // definition; the first estimate retains the exact guarded loader path.
    capture quietly program list __fereg_estimate_plugin
    if _rc {
        _fereg_load, action(loadonly)
        local plugin_file `"`r(plugin_file)'"'
        capture program __fereg_estimate_plugin, plugin using(`"`plugin_file'"')
        local load_rc = _rc
        if `load_rc' != 0 & `load_rc' != 110 exit `load_rc'
    }
    local deterministic_wire = (`"`deterministic'"' != "")
    quietly _fereg_memory_baseline
    local stata_used_before = strtrim(string(r(used), "%21.0f"))
    local stata_alloc_before = strtrim(string(r(allocated), "%21.0f"))
    local request_arguments `"ado=0.5.0;protocol_major=1;protocol_minor=6;stata=`c(stata_version)';confirm=fereg-estimation;plugin=0.5.0;result_major=1;result_minor=5;model_major=`model_major';model_minor=`model_minor';source_matrix=`model_sources';source_count=`n_sources';term_matrix=`model_terms';term_count=`n_terms';component_matrix=`model_components';component_count=`n_components';source_names=`source_registry';canonical_tokens=`token_registry';raw_component_operators=`raw_registry';raw_time_series_operators=`ts_registry';absorb_term_matrix=`absorb_terms';absorb_term_count=`n_absorb_terms';absorb_component_matrix=`absorb_components';absorb_component_count=`n_absorb_components';absorb_labels=`absorb_label_registry';absorb_saved_names=`absorb_saved_registry';cluster_term_matrix=`cluster_terms';cluster_term_count=`n_cluster_terms';cluster_component_matrix=`cluster_components';cluster_component_count=`n_cluster_components';cluster_labels=`cluster_label_registry';binding_matrix=`input_bindings';binding_count=`n_bindings';include_intercept=`include_intercept';candidate_all=`candidate_all';input_complete=`input_complete';keep_singletons=`keep_singletons';vce=`vce_wire';dof=`dof_wire';account_cluster_nesting=`account_cluster_nesting';memory=`memory_mode';solver=`solver_mode';preconditioner=`preconditioner_mode';poolsize=`poolsize';threads=`threads';deterministic=`deterministic_wire';tolerance=`tolerance';iterate=`iterate';output_sample=`output_sample';output_residual=`output_residual';output_drop_reason=0;output_xb=`output_xb';output_xbd=`output_xbd';output_absorbed=`output_absorbed';output_saved_count=`output_saved_count';output_saved_channels=`output_saved_registry';output_b_matrix=`b_shadow';output_v_matrix=`v_shadow';output_status_matrix=`status_shadow';output_marker_b_matrix=`marker_b_shadow';output_marker_v_matrix=`marker_v_shadow';output_fit_matrix=`fit_shadow';output_dof_matrix=`dof_shadow';output_solver_matrix=`solver_shadow';output_fe_matrix=`fe_shadow';output_cluster_matrix=`cluster_shadow';output_subset_matrix=`subset_shadow';output_certificate_matrix=`certificate_shadow';output_runtime_matrix=`runtime_shadow';output_memory_matrix=`memory_shadow';stata_used_entry=`stata_used_entry';stata_alloc_entry=`stata_alloc_entry';stata_used_before=`stata_used_before';stata_alloc_before=`stata_alloc_before';output_receipt_matrix=`receipt_shadow';"'
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
    local request_bytes = strlen(`"`request_bundle'"')
    if `request_bytes' > 4000 {
        local fereg_request_bundle `"`request_bundle'"'
        local request_bundle "bundle_local=`request_bytes'"
    }
    else local request_bundle `"bundle=`request_bundle'"'
    plugin call __fereg_estimate_plugin `candidate_variable' `depvar' `source_vars' ///
        `weight_variable' `output_variables', ///
        `"estimate-ols"' `"`request_bundle'"'

    if `"`inject'"' == "breakpostplugin" {
        di as result "FEREG_BREAK_ARMED"
        sleep 30000
        exit 3498
    }
    if `"`inject'"' == "postplugin" exit 3498

    _fereg_memory_validate `memory_shadow'
    _fereg_eols_validate `b_shadow' `v_shadow' ///
        `status_shadow' `marker_b_shadow' `marker_v_shadow' ///
        `fit_shadow' `dof_shadow' `solver_shadow' ///
        `fe_shadow' `cluster_shadow' `subset_shadow' ///
        `certificate_shadow' `runtime_shadow' `receipt_shadow', ///
        capacity(`coefficient_capacity') vcecode(`vce_code') ///
        absorbrows(`fe_rows') clusters(`n_cluster_terms') ///
        subsets(`subset_rows') residual(`=(`"`residual'"' != "")')

    local coefficient_count = r(coefficients)
    if `coefficient_count' > 0 & `coefficient_count' < `coefficient_capacity' {
        matrix `b_shadow' = `b_shadow'[1, 1..`coefficient_count']
        matrix `v_shadow' = `v_shadow'[1..`coefficient_count', 1..`coefficient_count']
        matrix `status_shadow' = `status_shadow'[1..`coefficient_count', 1..4]
        matrix `marker_b_shadow' = `marker_b_shadow'[1, 1..`coefficient_count']
        matrix `marker_v_shadow' = `marker_v_shadow'[1..`coefficient_count', 1..`coefficient_count']
    }

    // Plain continuous tokens cannot change when singleton deletion rebases
    // the final sample. Reuse the parser's canonical names and reserve the
    // second observation-scale fvexpand for ordinary categorical components.
    local requires_final_fvexpand 0
    if `n_components' > 0 {
        forvalues component = 1/`n_components' {
            if `model_components'[`component',4] == 2 local requires_final_fvexpand 1
        }
    }
    if `requires_final_fvexpand' & strtrim(`"`indepvars'"') != "" {
        local coefficient_names
        fvexpand `indepvars' if `sample_shadow'
        local coefficient_names `"`r(varlist)'"'
    }
    local n_terms : word count `coefficient_names'
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
        tempname posted_b posted_v
        matrix colnames `marker_b_shadow' = `posted_names'
        matrix colnames `marker_v_shadow' = `posted_names'
        matrix rownames `marker_v_shadow' = `posted_names'
        _ms_findomitted `marker_b_shadow' `marker_v_shadow'
        local marked_names : colfullnames `marker_b_shadow'
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
    ereturn scalar N_hdfe = `n_absorb_terms'
    ereturn scalar N_hdfe_extended = `fe_rows'
    ereturn scalar fereg_df_fit = `dof_shadow'[11,2]
    ereturn scalar fereg_df_correction = `dof_shadow'[12,2]
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
            local dof_row_name
            forvalues component = 1/`n_absorb_components' {
                if `absorb_components'[`component',1] == `term' & ///
                   `absorb_components'[`component',2] == 1 {
                    local source_id = `absorb_components'[`component',4]
                    local source : word `source_id' of `source_vars'
                    if `"`dof_row_name'"' == "" local dof_row_name `"1.`source'"'
                    else local dof_row_name `"`dof_row_name'#1.`source'"'
                }
            }
            if `fe_shadow'[`row',1] == 4 {
                local slope_source_id = `fe_shadow'[`row',3]
                local slope_source : word `slope_source_id' of `source_vars'
                local dof_row_name `"`dof_row_name'#c.`slope_source'"'
            }
        }
        local dof_row_names `"`dof_row_names' `dof_row_name'"'
    }
    matrix rownames `posted_dof_table' = `dof_row_names'
    mata: st_matrixcolstripe("`posted_dof_table'", (J(5,1,""), ("Categories" \ "Redundant" \ "Num Coefs" \ "Inexact?" \ "Nested?")))
    ereturn matrix dof_table = `posted_dof_table', copy
    ereturn matrix fereg_status = `status_shadow', copy
    ereturn matrix fereg_fit = `fit_shadow', copy
    ereturn matrix fereg_dof = `dof_shadow', copy
    ereturn matrix fereg_solver = `solver_shadow', copy
    ereturn matrix fereg_fe = `fe_shadow', copy
    ereturn matrix fereg_cluster = `cluster_shadow', copy
    ereturn matrix fereg_subset = `subset_shadow', copy
    ereturn matrix fereg_certificate = `certificate_shadow', copy
    ereturn matrix fereg_runtime = `runtime_shadow', copy
    matrix rownames `memory_shadow' = schema_version estimate_status ///
        stata_used_before stata_alloc_before native_live_before native_peak_est ///
        stata_growth_est combined_peak_est additional_reserve safety_headroom model_gap_bits
    matrix colnames `memory_shadow' = present value
    ereturn matrix fereg_memory = `memory_shadow', copy
    ereturn local fereg_memory_method "phase-capacity-partial-v2"
    if `memory_shadow'[2,2] == 2 ereturn local fereg_memory_method "phase-capacity-calibrated-v2"
    ereturn matrix fereg_receipt = `receipt_shadow', copy

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
    ereturn local cmd "_fereg_estimate_ols"
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
    ereturn local predict "fereg_p"
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
    ereturn local fereg_action_protocol "1.6"
    ereturn local fereg_model_wire "2.1"
    ereturn local fereg_result_wire "1.5"
    ereturn local fereg_plugin "0.5.0"
    if `"`inject'"' == "postdiagnostics" exit 3498
    if `"`inject'"' == "precommit" exit 3498

    nobreak {
        if `"`residual'"' != "" rename `residual_shadow' `residual'
        if `"`xb'"' != "" rename `xb_shadow' `xb'
        if `"`xbd'"' != "" rename `xbd_shadow' `xbd'
        if `"`d'"' != "" rename `d_shadow' `d'
        local n_saved : word count `saved_target_variables'
        if `n_saved' > 0 {
            forvalues saved_index = 1/`n_saved' {
                local saved_shadow : word `saved_index' of `saved_shadow_variables'
                local saved_target : word `saved_index' of `saved_target_variables'
                rename `saved_shadow' `saved_target'
            }
        }
        _estimates unhold `held_estimates', not
    }
end


program define _fereg_eols_validate, rclass
    version 18.0
    syntax namelist(min=14 max=14), CAPACITY(integer) VCECODE(integer) ABSORBROWS(integer) ///
        CLUSTERS(integer) SUBSETS(integer) RESIDUAL(integer)
    local b : word 1 of `namelist'
    local V : word 2 of `namelist'
    local status : word 3 of `namelist'
    local marker_b : word 4 of `namelist'
    local marker_V : word 5 of `namelist'
    local fit : word 6 of `namelist'
    local dof : word 7 of `namelist'
    local solver : word 8 of `namelist'
    local fe : word 9 of `namelist'
    local clusters_table : word 10 of `namelist'
    local subsets_table : word 11 of `namelist'
    local certificates : word 12 of `namelist'
    local runtime : word 13 of `namelist'
    local receipt : word 14 of `namelist'
    tempname runtime_valid

    local physical = max(`capacity', 1)
    local expected_subsets = cond(`clusters' == 0, 0, `subsets')
    if rowsof(`b') != 1 | colsof(`b') != `physical' | ///
       rowsof(`V') != `physical' | colsof(`V') != `physical' | ///
       rowsof(`status') != `physical' | colsof(`status') != 4 | ///
       rowsof(`marker_b') != 1 | colsof(`marker_b') != `physical' | ///
       rowsof(`marker_V') != `physical' | colsof(`marker_V') != `physical' | ///
       rowsof(`fit') != 12 | colsof(`fit') != 2 | ///
       rowsof(`dof') != 20 | colsof(`dof') != 2 | ///
       rowsof(`solver') != 2 | colsof(`solver') != 16 | ///
       rowsof(`fe') != `absorbrows' | colsof(`fe') != 8 | ///
       rowsof(`clusters_table') != max(`clusters', 1) | colsof(`clusters_table') != 4 | ///
       rowsof(`subsets_table') != `subsets' | colsof(`subsets_table') != 4 | ///
       rowsof(`certificates') != 32 | colsof(`certificates') != 2 | ///
       rowsof(`runtime') != 30 | colsof(`runtime') != 2 | ///
       rowsof(`receipt') != 1 | colsof(`receipt') != 16 {
        di as error "hidden OLS result layout failed exact structural validation"
        exit 459
    }
    local coefficients = `receipt'[1,5]
    if `receipt'[1,1] != 5354601 | `receipt'[1,2] != 1 | ///
       `receipt'[1,3] != 5 | `receipt'[1,4] != `vcecode' | ///
       `coefficients' != int(`coefficients') | `coefficients' < 0 | ///
       `coefficients' > `capacity' | ///
       `receipt'[1,7] != _N | `receipt'[1,10] != `clusters' | ///
       `receipt'[1,11] != `absorbrows' | ///
       `receipt'[1,12] != `expected_subsets' | ///
       `receipt'[1,13] != 1 | `receipt'[1,14] != `residual' | ///
       `receipt'[1,15] != 1 | `receipt'[1,16] != 1 | ///
       `receipt'[1,6] < 0 | `receipt'[1,6] > `coefficients' | ///
       `receipt'[1,8] < 0 | `receipt'[1,8] > _N | ///
       `receipt'[1,9] < 0 | `receipt'[1,9] > `receipt'[1,8] {
       di as error "hidden OLS receipt failed exact structural validation"
       exit 459
    }
    mata: __fereg_runtime_validate = st_matrix("`runtime'")
    mata: st_numscalar("`runtime_valid'", ///
        all(((__fereg_runtime_validate[.,1] :== 0) :| ///
             (__fereg_runtime_validate[.,1] :== 1)) :& ///
            (__fereg_runtime_validate[.,2] :< .) :& ///
            ((__fereg_runtime_validate[.,1] :== 1) :| ///
             (__fereg_runtime_validate[.,2] :== 0))))
    mata: mata drop __fereg_runtime_validate
    if `runtime_valid' != 1 {
        di as error "hidden OLS runtime report failed defined/value validation"
        exit 459
    }
    if `runtime'[1,1] != 1 | `runtime'[2,1] != 1 | ///
       `runtime'[3,1] != 1 | `runtime'[4,1] != 1 | ///
       `runtime'[5,1] != 1 | `runtime'[7,1] != 1 | ///
       `runtime'[9,1] != 1 | `runtime'[10,1] != 1 | ///
       `runtime'[11,1] != 1 | `runtime'[12,1] != 1 | ///
       `runtime'[13,1] != 1 | `runtime'[18,1] != 1 | ///
       `runtime'[19,1] != 1 | `runtime'[20,1] != 1 | ///
       `runtime'[21,1] != 1 | `runtime'[22,1] != 1 | ///
       `runtime'[23,1] != 1 | `runtime'[24,1] != 1 | ///
       `runtime'[25,1] != 1 | `runtime'[28,1] != 1 | ///
       `runtime'[29,1] != 1 | `runtime'[30,1] != 1 {
        di as error "hidden OLS runtime report omitted a mandatory field"
        exit 459
    }
    if !inlist(`runtime'[1,2], 1, 2, 3) | ///
       !inlist(`runtime'[2,2], 1, 2) | ///
       `runtime'[3,2] < 1 | `runtime'[3,2] != int(`runtime'[3,2]) | ///
       `runtime'[4,2] < 0 | `runtime'[5,2] < 0 | `runtime'[9,2] < 0 | ///
       `runtime'[13,2] < 0 | `runtime'[18,2] < 0 | ///
       `runtime'[19,2] < 0 | `runtime'[20,2] < 0 | `runtime'[21,2] < 0 | ///
       `runtime'[28,2] < 0 | `runtime'[28,2] != int(`runtime'[28,2]) | ///
       `runtime'[29,2] < 1 | `runtime'[29,2] != int(`runtime'[29,2]) | ///
       !inlist(`runtime'[30,2], 0, 1) {
        di as error "hidden OLS runtime report failed semantic validation"
        exit 459
    }
    return scalar coefficients = `coefficients'
end
