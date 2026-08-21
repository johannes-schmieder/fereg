*! StFixest metadata-only parser prototype 0.1.0 20aug2026
program define stfixest_parse, rclass
    version 18.0
    syntax [varlist(fv ts numeric)] [if] [in], ///
        [ABSorb(string asis) CLUster(string asis)]

    local rhs `"`varlist'"'
    marksample touse, novarlist

    tempname term_meta component_meta absorb_meta absorb_component_meta
    tempname cluster_meta cluster_component_meta
    local term_rows 0
    local component_rows 0
    local absorb_rows 0
    local absorb_component_rows 0
    local cluster_rows 0
    local cluster_component_rows 0
    local source_vars
    local coefficient_names
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
            local type_code 9
            if `"`rtype'"' == "variable" local type_code 1
            if `"`rtype'"' == "factor" local type_code 2
            if `"`rtype'"' == "interaction" local type_code 3
            if `"`rtype'"' == "product" local type_code 4

            local omit 0
            capture local omit = r(omit)
            if missing(`omit') local omit 0

            local n_components 1
            if inlist(`"`rtype'"', "interaction", "product") {
                local n_components = r(k_names)
            }
            local any_base 0
            local any_ts_pending 0

            forvalues component = 1/`n_components' {
                if inlist(`"`rtype'"', "interaction", "product") {
                    local source `"`r(name`component')'"'
                    local ts_op `"`r(ts_op`component')'"'
                }
                else {
                    local source `"`r(name)'"'
                    local ts_op `"`r(ts_op)'"'
                }
                if `"`source'"' == "" local source `"`token'"'

                local source_position : list posof `"`source'"' in source_vars
                if `source_position' == 0 {
                    local source_vars `"`source_vars' `source'"'
                    local source_position : word count `source_vars'
                }

                local component_kind 1
                local base 0
                local level .
                if `"`rtype'"' == "factor" {
                    local component_kind 2
                    local base = r(base)
                    local level = r(level)
                }
                else if inlist(`"`rtype'"', "interaction", "product") {
                    capture local level = r(level`component')
                    if !_rc & !missing(`level') {
                        local component_kind 2
                        local base = r(base`component')
                    }
                    else {
                        local level .
                    }
                }
                if `base' local any_base 1

                /* Phase 0 only marks TS operators; it does not materialize them. */
                local ts_pending = (`"`ts_op'"' != "")
                if `ts_pending' local any_ts_pending 1

                matrix `component_meta' = nullmat(`component_meta') \ ///
                    (`term_order', `component', `source_position', `component_kind', ///
                     `level', `base', `omit', `ts_pending')
                local ++component_rows
            }

            matrix `term_meta' = nullmat(`term_meta') \ ///
                (`term_order', `type_code', `n_components', `omit', `any_base', ///
                 `term_order', `any_ts_pending', 0)
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

    if `term_rows' == 0 matrix `term_meta' = J(1, 8, .)
    if `component_rows' == 0 matrix `component_meta' = J(1, 8, .)
    if `absorb_rows' == 0 matrix `absorb_meta' = J(1, 4, .)
    if `absorb_component_rows' == 0 matrix `absorb_component_meta' = J(1, 4, .)
    if `cluster_rows' == 0 matrix `cluster_meta' = J(1, 2, .)
    if `cluster_component_rows' == 0 matrix `cluster_component_meta' = J(1, 3, .)

    matrix colnames `term_meta' = term type n_components omitted base coef_col ts_pending reserved
    matrix colnames `component_meta' = term component source kind level base omitted ts_pending
    matrix colnames `absorb_meta' = term n_categorical intercept n_slopes
    matrix colnames `absorb_component_meta' = term role order source
    matrix colnames `cluster_meta' = term n_sources
    matrix colnames `cluster_component_meta' = term order source

    return scalar protocol_major = 1
    return scalar protocol_minor = 0
    return scalar n_model_terms = `term_rows'
    return scalar n_model_components = `component_rows'
    return scalar n_absorb_terms = `absorb_rows'
    return scalar n_absorb_components = `absorb_component_rows'
    return scalar n_cluster_terms = `cluster_rows'
    return scalar n_cluster_components = `cluster_component_rows'
    return local touse "`touse'"
    return local source_vars `"`source_vars'"'
    return local coefficient_names `"`coefficient_names'"'
    return local absorb_labels `"`absorb_labels'"'
    return local absorb_saved_names `"`absorb_saved_names'"'
    return local cluster_labels `"`cluster_labels'"'
    return matrix model_terms = `term_meta'
    return matrix model_components = `component_meta'
    return matrix absorb_terms = `absorb_meta'
    return matrix absorb_components = `absorb_component_meta'
    return matrix cluster_terms = `cluster_meta'
    return matrix cluster_components = `cluster_component_meta'
end
