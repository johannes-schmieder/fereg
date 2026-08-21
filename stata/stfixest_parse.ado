*! StFixest model-wire v2 metadata parser 0.2.0 21aug2026
program define stfixest_parse, rclass
    version 18.0
    syntax [varlist(fv ts numeric)], ///
        TOUSE(varname numeric) [ABSorb(string asis) CLUster(string asis) EMPTYMODEL]

    local rhs `"`varlist'"'
    if `"`emptymodel'"' != "" local rhs
    confirm variable `touse'

    tempname source_meta term_meta component_meta absorb_meta absorb_component_meta
    tempname cluster_meta cluster_component_meta
    local term_rows 0
    local component_rows 0
    local absorb_rows 0
    local absorb_component_rows 0
    local cluster_rows 0
    local cluster_component_rows 0
    local source_vars
    local coefficient_names
    local component_raw_operators
    local component_ts_operators
    local absorb_labels
    local absorb_saved_names
    local cluster_labels

    /* Ordinary factor-variable terms: expand names only; do not call fvrevar. */
    if strtrim(`"`rhs'"') != "" {
        quietly fvexpand `rhs' if `touse'
        local expanded `"`r(varlist)'"'
        local term_order 0
        foreach token of local expanded {
            local ++term_order
            quietly _ms_parse_parts `token'
            local rtype `"`r(type)'"'
            local type_code 0
            if `"`rtype'"' == "variable" local type_code 1
            else if `"`rtype'"' == "factor" local type_code 2
            else if `"`rtype'"' == "interaction" local type_code 3
            else if `"`rtype'"' == "product" local type_code 4
            else {
                di as error "unsupported _ms_parse_parts term type: `rtype'"
                exit 198
            }

            local omit 0
            capture local omit = r(omit)
            if missing(`omit') local omit 0

            local n_components 1
            if inlist(`"`rtype'"', "interaction", "product") {
                local n_components = r(k_names)
            }
            forvalues component = 1/`n_components' {
                if inlist(`"`rtype'"', "interaction", "product") {
                    local source `"`r(name`component')'"'
                    local ts_op `"`r(ts_op`component')'"'
                    local raw_op `"`r(op`component')'"'
                }
                else {
                    local source `"`r(name)'"'
                    local ts_op `"`r(ts_op)'"'
                    local raw_op `"`r(op)'"'
                }
                if `"`source'"' == "" {
                    di as error "missing component source metadata for `token'"
                    exit 198
                }

                local source_position : list posof `"`source'"' in source_vars
                if `source_position' == 0 {
                    local source_vars `"`source_vars' `source'"'
                    local source_position : word count `source_vars'
                }

                local component_kind 1
                local level_state 1
                local level 0
                local base_status 0
                local component_omit_status 2
                if `"`rtype'"' == "factor" {
                    local component_kind 2
                    local level_state 2
                    local level = r(level)
                    local base_status = r(base)
                    local component_omit_status = r(omit)
                }
                else if inlist(`"`rtype'"', "interaction", "product") {
                    local captured_level .
                    capture local captured_level = r(level`component')
                    if !_rc & !missing(`captured_level') {
                        local component_kind 2
                        local level_state 2
                        local level = `captured_level'
                        local captured_base .
                        capture local captured_base = r(base`component')
                        if !_rc & !missing(`captured_base') local base_status = `captured_base'
                        else local base_status 2
                    }
                    else if inlist(`"`raw_op'"', "c", "co") {
                        local component_kind 1
                        local level_state 1
                        local level 0
                        local base_status 0
                    }
                    else {
                        local component_kind 3
                        local level_state 3
                        local level 0
                        local base_status 2
                    }
                }

                /* This parser records TS work but never materializes it. */
                local ts_state 1
                if `"`ts_op'"' != "" local ts_state 2

                local ++component_rows
                local raw_op_length = strlen(`"`raw_op'"')
                local ts_op_length = strlen(`"`ts_op'"')
                local component_raw_operators ///
                    `"`component_raw_operators' `raw_op_length':`raw_op'"'
                local component_ts_operators ///
                    `"`component_ts_operators' `ts_op_length':`ts_op'"'

                matrix `component_meta' = nullmat(`component_meta') \ ///
                    (`term_order', `component', `source_position', `component_kind', ///
                     `level_state', `level', `base_status', `component_omit_status', ///
                     `ts_state', `component_rows', `component_rows', 0, 0)
            }

            matrix `term_meta' = nullmat(`term_meta') \ ///
                (`term_order', `term_order', `term_order', `type_code', ///
                 `n_components', `omit', 0, 0)
            local ++term_rows
            local coefficient_names `"`coefficient_names' `token'"'
        }
    }

    /* Absorbed terms. gettoken, bind keeps c.(slope1 slope2) together. */
    local absorb_rest `"`absorb'"'
    while strtrim(`"`absorb_rest'"') != "" {
        gettoken raw_absorb absorb_rest : absorb_rest, bind
        if `"`raw_absorb'"' == "" continue
        local ++absorb_rows

        local saved_name
        local equal_position = strpos(`"`raw_absorb'"', "=")
        if `equal_position' > 0 {
            local saved_name = substr(`"`raw_absorb'"', 1, `equal_position' - 1)
            local raw_absorb = substr(`"`raw_absorb'"', `equal_position' + 1, .)
            capture confirm new variable `saved_name'
            if _rc {
                di as error "invalid saved fixed-effect variable in absorb(): `saved_name'"
                exit 198
            }
        }

        local intercept 1
        local categorical_part `"`raw_absorb'"'
        local slope_part
        local split_position = strpos(`"`raw_absorb'"', "##c.")
        if `split_position' > 0 {
            local categorical_part = substr(`"`raw_absorb'"', 1, `split_position' - 1)
            local slope_part = substr(`"`raw_absorb'"', `split_position' + 4, .)
            local intercept 1
        }
        else {
            local split_position = strpos(`"`raw_absorb'"', "#c.")
            if `split_position' > 0 {
                local categorical_part = substr(`"`raw_absorb'"', 1, `split_position' - 1)
                local slope_part = substr(`"`raw_absorb'"', `split_position' + 3, .)
                local intercept 0
            }
        }

        if strpos(`"`categorical_part'"', "##") {
            di as error "categorical ## expansion inside absorb() is not represented by this Phase 0 parser"
            exit 198
        }
        local categorical_words : subinstr local categorical_part "#" " ", all
        local n_categorical : word count `categorical_words'
        if `n_categorical' == 0 {
            di as error "absorbed term has no categorical source: `raw_absorb'"
            exit 198
        }

        local categorical_order 0
        foreach categorical of local categorical_words {
            local ++categorical_order
            quietly _ms_parse_parts `categorical'
            local source `"`r(name)'"'
            if `"`source'"' == "" local source `"`categorical'"'
            capture confirm variable `source'
            if _rc {
                di as error "absorbed categorical source not found: `source'"
                exit 111
            }
            local source_position : list posof `"`source'"' in source_vars
            if `source_position' == 0 {
                local source_vars `"`source_vars' `source'"'
                local source_position : word count `source_vars'
            }
            matrix `absorb_component_meta' = nullmat(`absorb_component_meta') \ ///
                (`absorb_rows', 1, `categorical_order', `source_position')
            local ++absorb_component_rows
        }

        local slope_part = strtrim(`"`slope_part'"')
        if substr(`"`slope_part'"', 1, 1) == "(" & ///
           substr(`"`slope_part'"', strlen(`"`slope_part'"'), 1) == ")" {
            local slope_part = substr(`"`slope_part'"', 2, strlen(`"`slope_part'"') - 2)
        }
        local n_slopes 0
        local slope_order 0
        foreach slope of local slope_part {
            local ++slope_order
            local ++n_slopes
            if substr(`"`slope'"', 1, 2) == "c." local slope = substr(`"`slope'"', 3, .)
            capture confirm numeric variable `slope'
            if _rc {
                di as error "absorbed slope source must be numeric: `slope'"
                exit 109
            }
            local source_position : list posof `"`slope'"' in source_vars
            if `source_position' == 0 {
                local source_vars `"`source_vars' `slope'"'
                local source_position : word count `source_vars'
            }
            matrix `absorb_component_meta' = nullmat(`absorb_component_meta') \ ///
                (`absorb_rows', 2, `slope_order', `source_position')
            local ++absorb_component_rows
        }

        if !`intercept' & `n_slopes' == 0 {
            di as error "slope-only absorbed term has no slope: `raw_absorb'"
            exit 198
        }
        matrix `absorb_meta' = nullmat(`absorb_meta') \ ///
            (`absorb_rows', `n_categorical', `intercept', `n_slopes')
        local absorb_labels `"`absorb_labels' `raw_absorb'"'
        local absorb_saved_names `"`absorb_saved_names' `saved_name'"'
    }

    /* Cluster interactions are represented as tuples of original sources. */
    local cluster_rest `"`cluster'"'
    while strtrim(`"`cluster_rest'"') != "" {
        gettoken raw_cluster cluster_rest : cluster_rest, bind
        if `"`raw_cluster'"' == "" continue
        if strpos(`"`raw_cluster'"', "##") {
            di as error "cluster interactions use #, not ##"
            exit 198
        }
        local ++cluster_rows
        local cluster_words : subinstr local raw_cluster "#" " ", all
        local n_cluster_sources : word count `cluster_words'
        local cluster_order 0
        foreach cluster_source of local cluster_words {
            local ++cluster_order
            quietly _ms_parse_parts `cluster_source'
            local source `"`r(name)'"'
            if `"`source'"' == "" local source `"`cluster_source'"'
            capture confirm variable `source'
            if _rc {
                di as error "cluster source not found: `source'"
                exit 111
            }
            local source_position : list posof `"`source'"' in source_vars
            if `source_position' == 0 {
                local source_vars `"`source_vars' `source'"'
                local source_position : word count `source_vars'
            }
            matrix `cluster_component_meta' = nullmat(`cluster_component_meta') \ ///
                (`cluster_rows', `cluster_order', `source_position')
            local ++cluster_component_rows
        }
        matrix `cluster_meta' = nullmat(`cluster_meta') \ ///
            (`cluster_rows', `n_cluster_sources')
        local cluster_labels `"`cluster_labels' `raw_cluster'"'
    }

    local source_rows : word count `source_vars'
    if `source_rows' > 0 {
        forvalues source_id = 1/`source_rows' {
            matrix `source_meta' = nullmat(`source_meta') \ (`source_id', 0)
        }
    }

    /* Count zero is encoded by exactly one all-zero row of the documented width. */
    if `source_rows' == 0 matrix `source_meta' = J(1, 2, 0)
    if `term_rows' == 0 matrix `term_meta' = J(1, 8, 0)
    if `component_rows' == 0 matrix `component_meta' = J(1, 13, 0)
    if `absorb_rows' == 0 matrix `absorb_meta' = J(1, 4, 0)
    if `absorb_component_rows' == 0 matrix `absorb_component_meta' = J(1, 4, 0)
    if `cluster_rows' == 0 matrix `cluster_meta' = J(1, 2, 0)
    if `cluster_component_rows' == 0 matrix `cluster_component_meta' = J(1, 3, 0)

    matrix colnames `source_meta' = source reserved
    matrix colnames `term_meta' = term order coef_slot raw_type n_components omitted reserved1 reserved2
    matrix colnames `component_meta' = term component source kind level_state level base_status omit_status ts_state raw_op_slot ts_op_slot reserved1 reserved2
    matrix colnames `absorb_meta' = term n_categorical intercept n_slopes
    matrix colnames `absorb_component_meta' = term role order source
    matrix colnames `cluster_meta' = term n_sources
    matrix colnames `cluster_component_meta' = term order source

    return scalar model_wire_major = 2
    return scalar model_wire_minor = 0
    return scalar n_sources = `source_rows'
    return scalar n_model_terms = `term_rows'
    return scalar n_model_components = `component_rows'
    return scalar n_absorb_terms = `absorb_rows'
    return scalar n_absorb_components = `absorb_component_rows'
    return scalar n_cluster_terms = `cluster_rows'
    return scalar n_cluster_components = `cluster_component_rows'
    return local touse "`touse'"
    return local source_vars `"`source_vars'"'
    return local coefficient_names `"`coefficient_names'"'
    return local component_raw_operators `"`component_raw_operators'"'
    return local component_ts_operators `"`component_ts_operators'"'
    return local absorb_labels `"`absorb_labels'"'
    return local absorb_saved_names `"`absorb_saved_names'"'
    return local cluster_labels `"`cluster_labels'"'
    return matrix model_sources = `source_meta'
    return matrix model_terms = `term_meta'
    return matrix model_components = `component_meta'
    return matrix absorb_terms = `absorb_meta'
    return matrix absorb_components = `absorb_component_meta'
    return matrix cluster_terms = `cluster_meta'
    return matrix cluster_components = `cluster_component_meta'
end
