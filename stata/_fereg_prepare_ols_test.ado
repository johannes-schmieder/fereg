*! fereg developer-only numeric-factor OLS preparation boundary 0.5.0 24aug2026
program define _fereg_prepare_ols_test, rclass sortpreserve
    version 18.0
    syntax varlist(fv numeric min=1) [if] [in], ///
        ORIGINALOBS(name) RETAINED(name) DROPREASON(name) ///
        [NOCONStant AWEIGHT(varname numeric) FWEIGHT(varname numeric) ///
         PWEIGHT(varname numeric) ABSorb(string asis) KEEPsingletons MAPCHECK]

    local requested_weights = (`"`aweight'"' != "") + ///
        (`"`fweight'"' != "") + (`"`pweight'"' != "")
    if `requested_weights' > 1 {
        di as error "aweight(), fweight(), and pweight() are mutually exclusive"
        exit 198
    }

    foreach output in `originalobs' `retained' `dropreason' {
        confirm new variable `output'
    }

    marksample touse, novarlist
    gettoken depvar indepvars : varlist
    local absorb_option
    if strtrim(`"`absorb'"') != "" local absorb_option `"absorb(`absorb')"'
    tempname model_sources model_terms model_components absorb_terms
    tempname absorb_components input_bindings

    if strtrim(`"`indepvars'"') == "" {
        fereg_parse, touse(`touse') emptymodel `absorb_option'
    }
    else {
        fereg_parse `indepvars', touse(`touse') `absorb_option'
    }

    local source_vars `"`r(source_vars)'"'
    local coefficient_names `"`r(coefficient_names)'"'
    local component_raw `"`r(component_raw_operators)'"'
    local component_ts `"`r(component_ts_operators)'"'
    local n_sources = r(n_sources)
    local n_terms = r(n_model_terms)
    local n_components = r(n_model_components)
    local model_major = r(model_wire_major)
    local model_minor = r(model_wire_minor)
    local n_absorb_terms = r(n_absorb_terms)
    local n_absorb_components = r(n_absorb_components)
    local absorb_label_registry `"`r(absorb_label_registry)'"'
    local absorb_saved_name_registry `"`r(absorb_saved_name_registry)'"'
    matrix `model_sources' = r(model_sources)
    matrix `model_terms' = r(model_terms)
    matrix `model_components' = r(model_components)
    matrix `absorb_terms' = r(absorb_terms)
    matrix `absorb_components' = r(absorb_components)

    local n_absorb_intercepts 0
    if `n_absorb_terms' > 0 {
        forvalues row = 1/`n_absorb_terms' {
            local n_absorb_intercepts = `n_absorb_intercepts' + ///
                `absorb_terms'[`row',3]
        }
    }

    /* Stata's plugin varlist rejects strings before Rust can return r(498). */
    foreach source of local source_vars {
        capture confirm numeric variable `source'
        if _rc {
            di as error "prepare-ols-test does not support string model or fixed-effect sources"
            exit 498
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

    if `has_aweight' {
        local row = 3 + `n_sources'
        local position = 3 + `n_sources'
        matrix `input_bindings'[`row',1] = `position'
        matrix `input_bindings'[`row',2] = 4
    }
    if `has_fweight' {
        local row = 3 + `n_sources'
        local position = 3 + `n_sources'
        matrix `input_bindings'[`row',1] = `position'
        matrix `input_bindings'[`row',2] = 5
    }
    if `has_pweight' {
        local row = 3 + `n_sources'
        local position = 3 + `n_sources'
        matrix `input_bindings'[`row',1] = `position'
        matrix `input_bindings'[`row',2] = 6
    }

    generate double `originalobs' = .
    generate byte `retained' = .
    generate byte `dropreason' = .

    local output_original = 3 + `n_sources' + `has_weight'
    local output_retained = 4 + `n_sources' + `has_weight'
    local output_reason = 5 + `n_sources' + `has_weight'
    local include_intercept = (`"`noconstant'"' == "") & ///
        (`n_absorb_terms' == 0 | `n_absorb_intercepts' > 0)
    local keep_singletons = (`"`keepsingletons'"' != "")
    local run_borrowed_map = (`"`mapcheck'"' != "")
    local weight_variable `"`aweight'`fweight'`pweight'"'

    _fereg_load, action(loadonly)
    local plugin_file `"`r(plugin_file)'"'
    capture program __fereg_prepare_plugin, plugin using(`"`plugin_file'"')
    local load_rc = _rc
    if `load_rc' != 0 & `load_rc' != 110 exit `load_rc'
    plugin call __fereg_prepare_plugin `touse' `depvar' `source_vars' `weight_variable' ///
        `originalobs' `retained' `dropreason', ///
        `"prepare-ols-test"' ///
        `"ado=0.5.0"' ///
        `"protocol_major=1"' ///
        `"protocol_minor=0"' ///
        `"stata=`c(stata_version)'"' ///
        `"confirm=developer-preparation-test"' ///
        `"model_major=`model_major'"' ///
        `"model_minor=`model_minor'"' ///
        `"source_matrix=`model_sources'"' ///
        `"source_count=`n_sources'"' ///
        `"term_matrix=`model_terms'"' ///
        `"term_count=`n_terms'"' ///
        `"component_matrix=`model_components'"' ///
        `"component_count=`n_components'"' ///
        `"source_names=`source_registry'"' ///
        `"canonical_tokens=`token_registry'"' ///
        `"raw_component_operators=`raw_registry'"' ///
        `"raw_time_series_operators=`ts_registry'"' ///
        `"absorb_term_matrix=`absorb_terms'"' ///
        `"absorb_term_count=`n_absorb_terms'"' ///
        `"absorb_component_matrix=`absorb_components'"' ///
        `"absorb_component_count=`n_absorb_components'"' ///
        `"absorb_labels=`absorb_label_registry'"' ///
        `"absorb_saved_names=`absorb_saved_name_registry'"' ///
        `"binding_matrix=`input_bindings'"' ///
        `"binding_count=`n_bindings'"' ///
        `"include_intercept=`include_intercept'"' ///
        `"keep_singletons=`keep_singletons'"' ///
        `"run_borrowed_map=`run_borrowed_map'"' ///
        `"output_original_observation=`output_original'"' ///
        `"output_retained=`output_retained'"' ///
        `"output_drop_reason=`output_reason'"'

    return scalar n_original = _N
    quietly count if `retained' == 1
    return scalar n_retained = r(N)
    quietly count if `dropreason' == 8
    return scalar n_singletons = r(N)
    return scalar keep_singletons = `keep_singletons'
    return scalar map_checked = `run_borrowed_map'
    return local original_observation "`originalobs'"
    return local retained "`retained'"
    return local drop_reason "`dropreason'"
    local weight_type
    if `has_aweight' local weight_type "aweight"
    if `has_fweight' local weight_type "fweight"
    if `has_pweight' local weight_type "pweight"
    return local weight_type "`weight_type'"
    return local weight_variable "`weight_variable'"
end
