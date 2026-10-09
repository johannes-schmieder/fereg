*! fereg guarded PPML eclass transaction 0.5.0 02sep2026
program define _fereg_estimate_ppml, eclass
    version 18.0
    quietly _fereg_memory_baseline
    local stata_used_entry = strtrim(string(r(used), "%21.0f"))
    local stata_alloc_entry = strtrim(string(r(allocated), "%21.0f"))
    local hidden_cmdline `"_fereg_estimate_ppml `0'"'

    syntax varlist(fv ts numeric min=1) [if] [in], ///
        [FWEIGHT(varname numeric) PWEIGHT(varname numeric) ///
         ABSorb(string asis) CLUster(string asis) KEEPsingletons ///
         OFFset(varname numeric) EXPosure(varname numeric) ///
         BIASCORRection(string) BIASUNIT(varname numeric) BIASFE(name) ///
         BIASPROBes(string) BIASSEED(string) BIASSTREAM(string) ///
         VCE(string) SEParation(string asis) ///
         RESIDual(name) RESIDualtype(string) XB(name) D(name) ///
         ETA(name) XBD(name) MU(name) DROPreason(name) ///
         MEMory(string) POOLSize(integer 10) THREADS(integer 0) ///
         DETERMINISTIC SOLVer(string) ///
         TOLerance(real 1e-8) ITERate(integer 10000) ///
         SEPTOLerance(real 1e-8) SEPITERate(integer 10000) ///
         SIMPLEXITERate(integer 100000) SIMPLEXCELLS(integer 1000000) ///
         ALLOCATIONAUDIT INJECT(string) PUBLICMode ///
         PUBLICCMDLINE(string) PUBLICTIMEIT PUBLICDISPlay ///
         PUBLICWEXP(string)]

    local bias_method = lower(strtrim(`"`biascorrection'"'))
    local has_bias = (`"`bias_method'"' != "")
    if !`has_bias' & `"`biasunit'`biasfe'`biasprobes'`biasseed'`biasstream'"' != "" {
        di as error "bias options require biascorrection(exact or randomized)"
        exit 198
    }
    if `has_bias' {
        if !inlist(`"`bias_method'"', "exact", "randomized") | `"`biasunit'"' == "" {
            di as error "biascorrection() requires exact or randomized and explicit biasunit()"
            exit 198
        }
        if `"`bias_method'"' == "exact" & `"`biasprobes'`biasseed'`biasstream'"' != "" {
            di as error "probe, seed, and stream controls apply only to randomized correction"
            exit 198
        }
        if `"`biasprobes'"' == "" local biasprobes = cond(`"`bias_method'"' == "exact", 0, 256)
        if `"`biasseed'"' == "" local biasseed = cond(`"`bias_method'"' == "exact", 0, 739391)
        if `"`biasstream'"' == "" local biasstream 0
        foreach option in biasprobes biasseed biasstream {
            local value = real(`"``option''"')
            if missing(`value') | `value' != floor(`value') | !inrange(`value', 0, 4294967295) {
                di as error "`option'() must be an unsigned 32-bit integer"
                exit 198
            }
            local `option' = strtrim(string(`value', "%21.0f"))
        }
        if `"`bias_method'"' == "randomized" & `biasprobes' < 2 {
            di as error "randomized correction requires at least two probes"
            exit 198
        }
    }
    local bias_target_variables
    if `"`biasfe'"' != "" {
        forvalues effect = 1/2 {
            foreach channel in raw d c total bc sed sec covdc setotal {
                local target `biasfe'_`effect'_`channel'
                confirm new variable `target'
                local bias_target_variables `bias_target_variables' `target'
            }
        }
    }

    gettoken depvar_token : varlist
    capture confirm numeric variable `depvar_token'
    if _rc {
        di as error "PPML dependent variable must be a plain numeric variable"
        exit 198
    }

    if (`"`publicmode'"' == "" & ///
            (strtrim(`"`publiccmdline'"') != "" | `"`publictimeit'"' != "" | ///
             `"`publicdisplay'"' != "" | strtrim(`"`publicwexp'"') != "")) | ///
       (`"`publicmode'"' != "" & strtrim(`"`publiccmdline'"') == "") {
        di as error "invalid private PPML public-publication context"
        exit 198
    }

    local solver_mode = lower(strtrim(`"`solver'"'))
    if `"`solver_mode'"' == "" local solver_mode "auto"
    if !inlist(`"`solver_mode'"', "auto", "map", "lsmr", "cmg") {
        di as error "solver() must be auto, map, lsmr, or cmg"
        exit 198
    }
    local requested_solver_code = cond(`"`solver_mode'"' == "auto", 1, ///
        cond(`"`solver_mode'"' == "map", 2, cond(`"`solver_mode'"' == "lsmr", 3, 7)))

    local requested_weights = (`"`fweight'"' != "") + (`"`pweight'"' != "")
    if `requested_weights' > 1 {
        di as error "fweight() and pweight() are mutually exclusive"
        exit 198
    }
    if strtrim(`"`publicwexp'"') != "" & `requested_weights' == 0 {
        di as error "private publicwexp() requires a PPML weight"
        exit 198
    }
    if `"`offset'"' != "" & `"`exposure'"' != "" {
        di as error "offset() and exposure() are mutually exclusive"
        exit 198
    }
    if `poolsize' <= 0 | `threads' < 0 | ///
       `tolerance' <= 0 | `tolerance' >= 1 | `iterate' <= 0 | ///
       `septolerance' <= 0 | `septolerance' >= 1 | `sepiterate' <= 0 | ///
       `simplexiterate' <= 0 | `simplexcells' <= 0 {
        di as error "PPML tolerances must be in (0,1) and iteration limits must be positive"
        exit 198
    }

    local memory_mode = lower(strtrim(`"`memory'"'))
    if `"`memory_mode'"' == "" local memory_mode "auto"
    if !inlist(`"`memory_mode'"', "auto", "fast", "low") {
        di as error "memory() must be auto, fast, or low"
        exit 198
    }
    if inlist(`"`solver_mode'"', "lsmr", "cmg") & `"`memory_mode'"' == "low" {
        di as error "solver(`solver_mode') does not support memory(low) in this release"
        exit 498
    }
    if inlist(`"`solver_mode'"', "lsmr", "cmg") & `"`allocationaudit'"' != "" {
        di as error "solver(`solver_mode') does not support allocation auditing in this release"
        exit 498
    }

    local separation_wire = lower(strtrim(`"`separation'"'))
    if `"`separation_wire'"' == "" local separation_wire "fe simplex ir"
    local separation_word_count : word count `separation_wire'
    local separation_count 0
    local seen_separation
    foreach method of local separation_wire {
        if !inlist(`"`method'"', "fe", "simplex", "ir", "mu", "none", "default") {
            di as error "unknown separation method `method'"
            exit 198
        }
        if inlist(`"`method'"', "none", "default") & `separation_word_count' != 1 {
            di as error "separation(`method') cannot be combined with another method"
            exit 198
        }
        local duplicate : list method in seen_separation
        if `duplicate' {
            di as error "separation methods may not be repeated"
            exit 198
        }
        local seen_separation `seen_separation' `method'
        if !inlist(`"`method'"', "none", "default") local ++separation_count
    }
    if `"`separation_wire'"' == "default" local separation_count 3
    // The bounded report always includes singleton pruning as row one.
    local separation_count = 1 + `separation_count'
    local separation_codes 1
    if `"`separation_wire'"' == "default" {
        local separation_codes 1 2 3 4
    }
    else if `"`separation_wire'"' != "none" {
        foreach method of local separation_wire {
            if `"`method'"' == "fe" local separation_codes `separation_codes' 2
            if `"`method'"' == "simplex" local separation_codes `separation_codes' 3
            if `"`method'"' == "ir" local separation_codes `separation_codes' 4
            if `"`method'"' == "mu" local separation_codes `separation_codes' 5
        }
        local separation_codes : list sort separation_codes
    }

    local vce_text = strtrim(`"`vce'"')
    if `"`vce_text'"' == "" local vce_text "robust"
    gettoken vce_kind vce_rest : vce_text
    local vce_kind = lower(`"`vce_kind'"')
    local vce_rest = strtrim(`"`vce_rest'"')
    if `"`vce_kind'"' == "unadjusted" & `"`vce_rest'"' == "" {
        local vce_request_wire "unadjusted"
        local vce_wire "robust"
    }
    else if inlist(`"`vce_kind'"', "ols", "iid") & `"`vce_rest'"' == "" {
        di as error "vce(`vce_kind') is not defined for public PPML inference"
        exit 198
    }
    else if `"`vce_kind'"' == "robust" & `"`vce_rest'"' == "" {
        local vce_request_wire "robust"
        local vce_wire "robust"
    }
    else if `"`vce_kind'"' == "cluster" {
        if `"`vce_rest'"' == "" & strtrim(`"`cluster'"') == "" {
            di as error "vce(cluster) requires at least one cluster dimension"
            exit 198
        }
        if `"`vce_rest'"' == "" local vce_rest = strtrim(`"`cluster'"')
        if strtrim(`"`cluster'"') != "" & strtrim(`"`cluster'"') != `"`vce_rest'"' {
            di as error "cluster() and vce(cluster ...) disagree"
            exit 198
        }
        local cluster `"`vce_rest'"'
        local vce_request_wire "cluster"
        local vce_wire "cluster"
    }
    else {
        di as error "vce() must be robust, unadjusted, or cluster varlist"
        exit 198
    }
    if `"`vce_wire'"' != "cluster" & strtrim(`"`cluster'"') != "" {
        local vce_request_wire "cluster"
        local vce_wire "cluster"
    }

    local inject = lower(strtrim(`"`inject'"'))
    local inject_known = inlist(`"`inject'"', "", "posthold", "postplugin", ///
        "poststripe", "postereturn", "postscalars", "postpublic", ///
        "postdiagnostics", "precommit")
    local inject_known = `inject_known' | inlist(`"`inject'"', ///
        "breakpostplugin", "breakpostpublic", "mutatewire", ///
        "mutatesemantic", "mutatesolver", "mutateoutput", "panic", "breaknative", ///
        "allocation")
    if !`inject_known' {
        di as error "unknown developer PPML transaction injection stage: `inject'"
        exit 198
    }

    local residual_wire = lower(strtrim(`"`residualtype'"'))
    if `"`residual_wire'"' == "" {
        if `"`residual'"' == "" local residual_wire "none"
        else local residual_wire "response"
    }
    if !inlist(`"`residual_wire'"', "none", "response", "score", ///
        "pearson", "working", "deviance", "anscombe") {
        di as error "residualtype() must be response, score, pearson, working, deviance, or anscombe"
        exit 198
    }
    if (`"`residual_wire'"' == "none") != (`"`residual'"' == "") {
        di as error "residualtype(none) and residual() must be specified consistently"
        exit 198
    }
    if `"`eta'"' != "" & `"`xbd'"' != "" {
        di as error "eta() and xbd() are aliases and may not be combined"
        exit 198
    }
    local eta_target `"`eta'`xbd'"'

    local requested_output_names `residual' `xb' `d' `eta_target' `mu' `dropreason'
    local requested_output_names = strtrim(`"`requested_output_names'"')
    local unique_output_names : list uniq requested_output_names
    local requested_output_count : word count `requested_output_names'
    local unique_output_count : word count `unique_output_names'
    if `requested_output_count' != `unique_output_count' {
        di as error "PPML output variable names must be distinct"
        exit 110
    }
    * Preserve Stata's specific binding error (r(110)) for existing names.
    foreach output of local requested_output_names {
        confirm new variable `output'
    }

    marksample touse, novarlist
    local candidate_all = (strtrim(`"`if'"') == "" & strtrim(`"`in'"') == "")
    gettoken depvar indepvars : varlist
    local absorb_option
    local cluster_option
    if strtrim(`"`absorb'"') != "" local absorb_option `"absorb(`absorb')"'
    if strtrim(`"`cluster'"') != "" local cluster_option `"cluster(`cluster')"'

    * PPML's categorical FE and cluster owners accept original sources only.
    * The shared fereg parser intentionally has broader parsing behavior, so
    * reject time-series aliases here before it can strip r(ts_op).
    local ppml_absorb_rest `"`absorb'"'
    while strtrim(`"`ppml_absorb_rest'"') != "" {
        gettoken ppml_absorb_term ppml_absorb_rest : ppml_absorb_rest, bind
        local ppml_equal = strpos(`"`ppml_absorb_term'"', "=")
        if `ppml_equal' > 0 local ppml_absorb_term = ///
            substr(`"`ppml_absorb_term'"', `ppml_equal' + 1, .)
        local ppml_absorb_term : subinstr local ppml_absorb_term "##c." "#", all
        local ppml_absorb_term : subinstr local ppml_absorb_term "#c." "#", all
        local ppml_absorb_term : subinstr local ppml_absorb_term "#" " ", all
        local ppml_absorb_term : subinstr local ppml_absorb_term "(" " ", all
        local ppml_absorb_term : subinstr local ppml_absorb_term ")" " ", all
        foreach ppml_absorb_source of local ppml_absorb_term {
            quietly _ms_parse_parts `ppml_absorb_source'
            if strtrim(`"`r(ts_op)'"') != "" {
                di as error "time-series operators are not allowed in PPML absorb()"
                exit 198
            }
        }
    }
    local ppml_cluster_terms : subinstr local cluster "#" " ", all
    foreach ppml_cluster_source of local ppml_cluster_terms {
        quietly _ms_parse_parts `ppml_cluster_source'
        if strtrim(`"`r(ts_op)'"') != "" {
            di as error "time-series operators are not allowed in PPML clusters"
            exit 198
        }
    }

    local factor_sample "`touse'"
    if strtrim(`"`indepvars'"') != "" {
        tempvar factor_available
        local availability_options
        foreach option in fweight pweight offset exposure {
            if "``option''" != "" local availability_options ///
                `"`availability_options' `option'(``option'')"'
        }
        quietly _fereg_factor_sample `varlist', touse(`touse') generate(`factor_available') ///
            `absorb_option' `cluster_option' `availability_options'
        local factor_sample "`r(touse)'"
    }

    tempname model_sources model_terms model_components absorb_terms
    tempname absorb_components cluster_terms cluster_components input_bindings
    if strtrim(`"`indepvars'"') == "" {
        fereg_parse, touse(`touse') emptymodel ///
            `absorb_option' `cluster_option'
    }
    else {
        fereg_parse `indepvars', touse(`factor_sample') `absorb_option' `cluster_option'
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

    if `n_cluster_terms' > 5 {
        di as error "feppml supports at most five cluster dimensions"
        exit 198
    }
    if `"`vce_wire'"' == "cluster" & `n_cluster_terms' == 0 {
        di as error "vce(cluster) requires at least one cluster dimension"
        exit 198
    }

    // Stata, not Rust, owns time-series indexing and panel-gap semantics.
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

            local source_has_other_use 0
            forvalues check = 1/`n_components' {
                if `original_model_components'[`check',3] == `source_id' & ///
                   `original_model_components'[`check',9] != 2 local source_has_other_use 1
            }
            if `n_absorb_components' > 0 {
                forvalues check = 1/`n_absorb_components' {
                    if `absorb_components'[`check',4] == `source_id' local source_has_other_use 1
                }
            }
            if `n_cluster_components' > 0 {
                forvalues check = 1/`n_cluster_components' {
                    if `cluster_components'[`check',3] == `source_id' local source_has_other_use 1
                }
            }
            local source_already_reused 0
            if `component_row' > 1 {
                forvalues prior = 1/`=`component_row' - 1' {
                    if `original_model_components'[`prior',3] == `source_id' & ///
                       `original_model_components'[`prior',9] == 2 local source_already_reused 1
                }
            }
            if !`source_has_other_use' & !`source_already_reused' {
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

    local has_fweight = (`"`fweight'"' != "")
    local has_pweight = (`"`pweight'"' != "")
    local has_weight = `has_fweight' + `has_pweight'
    local has_adjustment = (`"`offset'`exposure'"' != "")
    local n_bindings = 2 + `n_sources' + `has_weight'
    matrix `input_bindings' = J(`n_bindings', 3, 0)
    matrix `input_bindings'[1,1] = 1
    matrix `input_bindings'[1,2] = 1
    matrix `input_bindings'[2,1] = 2
    matrix `input_bindings'[2,2] = 2
    if `n_sources' > 0 {
        forvalues source = 1/`n_sources' {
            local row = 2 + `source'
            matrix `input_bindings'[`row',1] = `row'
            matrix `input_bindings'[`row',2] = 3
            matrix `input_bindings'[`row',3] = `source'
        }
    }
    if `has_weight' {
        local row = 3 + `n_sources'
        matrix `input_bindings'[`row',1] = `row'
        if `has_fweight' matrix `input_bindings'[`row',2] = 5
        if `has_pweight' matrix `input_bindings'[`row',2] = 6
    }

    local adjustment_kind "none"
    local adjustment_variable
    local adjustment_position 0
    if `"`offset'"' != "" {
        local adjustment_kind "offset"
        local adjustment_variable `offset'
        local adjustment_position = `n_bindings' + 1
    }
    if `"`exposure'"' != "" {
        local adjustment_kind "exposure"
        local adjustment_variable `exposure'
        local adjustment_position = `n_bindings' + 1
    }
    local bias_unit_position = cond(`has_bias', `n_bindings' + `has_adjustment' + 1, 0)
    local input_variable_count = `n_bindings' + `has_adjustment' + `has_bias'

    local n_absorb_intercepts 0
    local n_absorb_slopes 0
    local has_absorb_interaction 0
    if `n_absorb_terms' > 0 {
        forvalues row = 1/`n_absorb_terms' {
            local n_absorb_intercepts = `n_absorb_intercepts' + `absorb_terms'[`row',3]
            local n_absorb_slopes = `n_absorb_slopes' + `absorb_terms'[`row',4]
            if `absorb_terms'[`row',2] > 1 | `absorb_terms'[`row',4] > 0 ///
                local has_absorb_interaction 1
        }
    }
    if `has_absorb_interaction' {
        if regexm(lower(`"`indepvars'"'), "(^|[ #])ib[^ #]*[.]") {
            di as error "categorical PPML does not support explicit RHS base operators with interacted or sloped absorption"
            exit 498
        }
        if `n_components' > 0 {
            forvalues row = 1/`n_components' {
                if `model_components'[`row',4] == 2 {
                    local source_id = `model_components'[`row',3]
                    local source : word `source_id' of `source_vars'
                    local fv_base : char `source'[_fv_base]
                    if `"`fv_base'"' != "" {
                        di as error "categorical PPML does not support fvset base overrides with interacted or sloped absorption"
                        exit 498
                    }
                }
            }
        }
    }
    if `has_bias' & (`n_absorb_terms' != 2 | `n_absorb_intercepts' != 2 | `n_absorb_slopes' != 0) {
        di as error "bias correction requires exactly two categorical intercept fixed effects"
        exit 198
    }
    local output_saved_count = `n_absorb_intercepts' + `n_absorb_slopes'
    local saved_target_variables
    if `n_absorb_terms' > 0 {
        forvalues term = 1/`n_absorb_terms' {
            local saved_base : word `term' of `absorb_saved_names'
            local intercept = `absorb_terms'[`term',3]
            local slopes = `absorb_terms'[`term',4]
            local channels = `intercept' + `slopes'
            if `"`saved_base'"' == "." continue
            forvalues channel = 1/`channels' {
                local target `"`saved_base'"'
                if !`intercept' | `channel' > 1 {
                    local slope_index = `channel' - `intercept'
                    local target `"`saved_base'_Slope`slope_index'"'
                }
                local saved_target_variables `saved_target_variables' `target'
            }
        }
    }
    local all_output_names `requested_output_names' `saved_target_variables' `bias_target_variables'
    local all_output_names = strtrim(`"`all_output_names'"')
    local unique_all_output_names : list uniq all_output_names
    local all_output_count : word count `all_output_names'
    local unique_all_output_count : word count `unique_all_output_names'
    if `all_output_count' != `unique_all_output_count' {
        di as error "PPML output and saved fixed-effect names must be distinct"
        exit 110
    }
    foreach output of local saved_target_variables {
        confirm new variable `output'
    }
    local requested_saved_count : word count `saved_target_variables'
    if `"`solver_mode'"' == "lsmr" & ///
       (`n_absorb_intercepts' < 2 | `n_absorb_slopes' != 0) {
        di as error "solver(lsmr) currently requires at least two categorical intercept fixed effects"
        exit 498
    }
    if `"`solver_mode'"' == "cmg" & ///
       (!inlist(`n_absorb_intercepts', 2, 3) | `n_absorb_slopes' != 0) {
        di as error "solver(cmg) currently requires two or three categorical intercept fixed effects"
        exit 498
    }
    local coefficient_capacity = `n_terms' + 1
    local matrix_dimension = max(`coefficient_capacity', 1)
    local fe_rows = max(`n_absorb_intercepts' + `n_absorb_slopes', 1)
    local cluster_rows = max(`n_cluster_terms', 1)
    local subset_rows = max(2^`n_cluster_terms' - 1, 1)
    local separation_rows = max(`separation_count', 1)

    tempvar sample_shadow
    generate byte `sample_shadow' = `touse'
    local output_variables `sample_shadow'
    local output_sample = `input_variable_count' + 1
    local output_residual 0
    local output_xb 0
    local output_absorbed 0
    local output_eta 0
    local output_mu 0
    local output_dropreason 0
    if `"`residual'"' != "" {
        tempvar residual_shadow
        generate double `residual_shadow' = .
        label variable `residual_shadow' "PPML `residual_wire' residual"
        local output_variables `output_variables' `residual_shadow'
        local output_variable_count : word count `output_variables'
        local output_residual = `input_variable_count' + `output_variable_count'
    }
    if `"`xb'"' != "" {
        tempvar xb_shadow
        generate double `xb_shadow' = .
        label variable `xb_shadow' "Finite linear prediction including adjustment"
        local output_variables `output_variables' `xb_shadow'
        local output_variable_count : word count `output_variables'
        local output_xb = `input_variable_count' + `output_variable_count'
    }
    if `"`d'"' != "" {
        tempvar d_shadow
        generate double `d_shadow' = .
        label variable `d_shadow' "Absorbed contribution"
        local output_variables `output_variables' `d_shadow'
        local output_variable_count : word count `output_variables'
        local output_absorbed = `input_variable_count' + `output_variable_count'
    }
    if `"`eta_target'"' != "" {
        tempvar eta_shadow
        generate double `eta_shadow' = .
        label variable `eta_shadow' "Total PPML linear prediction"
        local output_variables `output_variables' `eta_shadow'
        local output_variable_count : word count `output_variables'
        local output_eta = `input_variable_count' + `output_variable_count'
    }
    if `"`mu'"' != "" {
        tempvar mu_shadow
        generate double `mu_shadow' = .
        label variable `mu_shadow' "PPML conditional mean"
        local output_variables `output_variables' `mu_shadow'
        local output_variable_count : word count `output_variables'
        local output_mu = `input_variable_count' + `output_variable_count'
    }
    if `"`dropreason'"' != "" {
        tempvar dropreason_shadow
        generate long `dropreason_shadow' = .
        label variable `dropreason_shadow' "fereg PPML final-sample drop reason"
        local output_variables `output_variables' `dropreason_shadow'
        local output_variable_count : word count `output_variables'
        local output_dropreason = `input_variable_count' + `output_variable_count'
    }

    local output_saved_registry
    local saved_shadow_variables
    local saved_target_cursor 0
    if `n_absorb_terms' > 0 {
        forvalues term = 1/`n_absorb_terms' {
            local saved_base : word `term' of `absorb_saved_names'
            local intercept = `absorb_terms'[`term',3]
            local slopes = `absorb_terms'[`term',4]
            local channels = `intercept' + `slopes'
            forvalues channel = 1/`channels' {
                local saved_position 0
                if `"`saved_base'"' != "." {
                    local ++saved_target_cursor
                    local target : word `saved_target_cursor' of `saved_target_variables'
                    tempvar saved_shadow
                    generate double `saved_shadow' = .
                    label variable `saved_shadow' "[FE] `target'"
                    local output_variables `output_variables' `saved_shadow'
                    local output_variable_count : word count `output_variables'
                    local saved_position = `input_variable_count' + `output_variable_count'
                    local saved_shadow_variables `saved_shadow_variables' `saved_shadow'
                }
                local saved_piece `"`=strlen("`saved_position'")':`saved_position'"'
                if `"`output_saved_registry'"' == "" local output_saved_registry `"`saved_piece'"'
                else local output_saved_registry `"`output_saved_registry'|`saved_piece'"'
            }
        }
    }
    if `output_saved_count' == 0 local output_saved_registry "."

    local output_bias_registry "."
    local bias_shadow_variables
    local output_bias_count : word count `bias_target_variables'
    if `output_bias_count' {
        local output_bias_registry
        foreach target of local bias_target_variables {
            tempvar bias_shadow_variable
            generate double `bias_shadow_variable' = .
            label variable `bias_shadow_variable' "[PPML bias] `target'"
            local output_variables `output_variables' `bias_shadow_variable'
            local output_variable_count : word count `output_variables'
            local position = `input_variable_count' + `output_variable_count'
            local piece `"`=strlen("`position'")':`position'"'
            if `"`output_bias_registry'"' == "" local output_bias_registry `"`piece'"'
            else local output_bias_registry `"`output_bias_registry'|`piece'"'
            local bias_shadow_variables `bias_shadow_variables' `bias_shadow_variable'
        }
    }
    if `has_bias' {
        tempname bias_shadow bias_diagnostics_shadow bias_support_shadow
        matrix `bias_shadow' = J(9, `matrix_dimension', .)
        matrix `bias_diagnostics_shadow' = J(21, 1, .)
        matrix `bias_support_shadow' = J(2, 11, .)
    }

    tempname b_shadow v_shadow status_shadow fit_shadow dof_shadow
    tempname fe_shadow cluster_shadow subset_shadow separation_shadow
    tempname inference_shadow drop_counts_shadow separation_sequence_shadow
    tempname memory_shadow
    matrix `memory_shadow' = J(11, 2, .)
    tempname certificate_shadow solver_shadow runtime_shadow receipt_shadow
    matrix `b_shadow' = J(1, `matrix_dimension', .)
    matrix `v_shadow' = J(`matrix_dimension', `matrix_dimension', .)
    matrix `status_shadow' = J(`matrix_dimension', 4, .)
    matrix `fit_shadow' = J(27, 1, .)
    matrix `dof_shadow' = J(12, 1, .)
    matrix `fe_shadow' = J(`fe_rows', 8, .)
    matrix `cluster_shadow' = J(`cluster_rows', 4, .)
    matrix `subset_shadow' = J(`subset_rows', 4, .)
    matrix `separation_shadow' = J(`separation_rows', 14, .)
    matrix `inference_shadow' = J(16, 1, 0)
    matrix `drop_counts_shadow' = J(14, 2, 0)
    matrix `separation_sequence_shadow' = J(5, 13, 0)
    matrix `certificate_shadow' = J(16, 1, .)
    matrix `solver_shadow' = J(4, 22, .)
    matrix `runtime_shadow' = J(108, 1, .)
    matrix `receipt_shadow' = J(1, 33, 0)

    tempname held_estimates
    tempvar held_sample
    _estimates hold `held_estimates', restore nullok varname(`held_sample')
    if `"`inject'"' == "posthold" exit 3498

    local weight_variable `"`fweight'`pweight'"'
    * Native loading classifies missing values; completeness is an optional
    * transport hint, not sample authority. No ado copy/markout pass is needed.
    local input_complete 0

    _fereg_load, action(loadonly)
    local plugin_handle "`r(handle)'"
    local plugin_file `"`r(plugin_file)'"'
    // Stata plugin registrations are scoped to the calling ado program.
    capture program `plugin_handle', plugin using(`"`plugin_file'"')
    local load_rc = _rc
    if `load_rc' != 0 & `load_rc' != 110 exit `load_rc'

    local keep_singletons = (`"`keepsingletons'"' != "")
    local deterministic_wire = (`"`deterministic'"' != "")
    local allocation_audit_wire = (`"`allocationaudit'"' != "")
    local plugin_inject
    if `"`inject'"' == "panic" local plugin_inject "panic"
    if `"`inject'"' == "breaknative" local plugin_inject "breaknative"
    if `"`inject'"' == "allocation" local plugin_inject "allocation"
    quietly _fereg_memory_baseline
    local stata_used_before = strtrim(string(r(used), "%21.0f"))
    local stata_alloc_before = strtrim(string(r(allocated), "%21.0f"))
    local request_arguments `"ado=0.5.0;protocol_major=1;protocol_minor=6;stata=`c(stata_version)';confirm=feppml-estimation;plugin=0.5.0;result_major=1;result_minor=5;request_major=1;request_minor=8;model_major=`model_major';model_minor=`model_minor';source_matrix=`model_sources';source_count=`n_sources';term_matrix=`model_terms';term_count=`n_terms';component_matrix=`model_components';component_count=`n_components';source_names=`source_registry';canonical_tokens=`token_registry';raw_component_operators=`raw_registry';raw_time_series_operators=`ts_registry';absorb_term_matrix=`absorb_terms';absorb_term_count=`n_absorb_terms';absorb_component_matrix=`absorb_components';absorb_component_count=`n_absorb_components';absorb_labels=`absorb_label_registry';absorb_saved_names=`absorb_saved_registry';cluster_term_matrix=`cluster_terms';cluster_term_count=`n_cluster_terms';cluster_component_matrix=`cluster_components';cluster_component_count=`n_cluster_components';cluster_labels=`cluster_label_registry';binding_matrix=`input_bindings';binding_count=`n_bindings';include_intercept=1;candidate_all=`candidate_all';input_complete=`input_complete';keep_singletons=`keep_singletons';adjustment=`adjustment_kind';adjustment_position=`adjustment_position';residual=`residual_wire';vce=`vce_request_wire';memory=`memory_mode';poolsize=`poolsize';threads=`threads';deterministic=`deterministic_wire';allocation_audit=`allocation_audit_wire';solver=`solver_mode';tolerance=`tolerance';iterate=`iterate';separation=`separation_wire';separation_tolerance=`septolerance';separation_max_iterations=`sepiterate';simplex_max_iterations=`simplexiterate';simplex_max_tableau_cells=`simplexcells';output_sample=`output_sample';output_residual=`output_residual';output_xb=`output_xb';output_absorbed=`output_absorbed';output_eta=`output_eta';output_mean=`output_mu';output_drop_reason=`output_dropreason';output_saved_count=`output_saved_count';output_saved_channels=`output_saved_registry';output_b_matrix=`b_shadow';output_v_matrix=`v_shadow';output_status_matrix=`status_shadow';output_fit_matrix=`fit_shadow';output_dof_matrix=`dof_shadow';output_fe_matrix=`fe_shadow';output_cluster_matrix=`cluster_shadow';output_subset_matrix=`subset_shadow';output_separation_matrix=`separation_shadow';output_inference_matrix=`inference_shadow';output_drop_counts_matrix=`drop_counts_shadow';output_separation_sequence_matrix=`separation_sequence_shadow';output_certificate_matrix=`certificate_shadow';output_solver_matrix=`solver_shadow';output_runtime_matrix=`runtime_shadow';output_memory_matrix=`memory_shadow';stata_used_entry=`stata_used_entry';stata_alloc_entry=`stata_alloc_entry';stata_used_before=`stata_used_before';stata_alloc_before=`stata_alloc_before';output_receipt_matrix=`receipt_shadow';"'
    if `has_bias' {
        local request_arguments `"`request_arguments'bias_method=`bias_method';bias_unit=`bias_unit_position';bias_probes=`biasprobes';bias_seed=`biasseed';bias_stream=`biasstream';output_bias_count=`output_bias_count';output_bias_channels=`output_bias_registry';output_bias_matrix=`bias_shadow';output_bias_diagnostics=`bias_diagnostics_shadow';output_bias_support=`bias_support_shadow';"'
    }
    if `"`plugin_inject'"' != "" {
        local request_arguments `"`request_arguments'inject=`plugin_inject';"'
    }
    if `"`plugin_inject'"' == "breaknative" {
        local break_marker_token : environment FEREG_PPML_BREAK_MARKER_TOKEN
        if strtrim(`"`break_marker_token'"') == "" {
            di as error "hidden PPML breaknative requires FEREG_PPML_BREAK_MARKER_TOKEN"
            exit 198
        }
        local request_arguments ///
            `"`request_arguments'break_marker_token=`break_marker_token';"'
    }
    // Frame bounded parser metadata in one vectorized pass. No observations or
    // generated design columns enter Mata; field lengths remain UTF-8 bytes.
    tempname request_input request_parser request_parts
    // Mata externals outlive an aborted ado call. Release every constructed
    // prefix before propagating failure; the bounded metadata step defers Break.
    nobreak {
        capture noisily {
            mata: `request_input' = st_local("request_arguments")
            mata: `request_parser' = tokeninit(";", "", "")
            mata: tokenset(`request_parser', `request_input')
            mata: `request_parts' = strtrim(tokengetall(`request_parser'))
            mata: st_local("request_bundle", cols(`request_parts') ? strtrim(strofreal(cols(`request_parts'), "%21.0f")) + ":" + invtokens(strtrim(strofreal(strlen(`request_parts'), "%21.0f")) :+ ":" :+ `request_parts', "") : "0:")
        }
        local request_rc = _rc
        capture mata: mata drop `request_input'
        capture mata: mata drop `request_parser'
        capture mata: mata drop `request_parts'
    }
    if `request_rc' exit `request_rc'
    local request_arguments
    local request_bytes = strlen(`"`request_bundle'"')
    if `request_bytes' > 4000 {
        local fereg_request_bundle `"`request_bundle'"'
        local request_bundle "bundle_local=`request_bytes'"
    }
    else local request_bundle `"bundle=`request_bundle'"'
    plugin call `plugin_handle' `touse' `depvar' `source_vars' ///
        `weight_variable' `adjustment_variable' `biasunit' `output_variables', ///
        `"estimate-ppml"' `"`request_bundle'"'

    // Developer-only publication-boundary mutations prove that malformed
    // native products are classified and rolled back without a second call.
    if `"`inject'"' == "mutatewire" matrix `receipt_shadow'[1,1] = 0
    if `"`inject'"' == "mutatesemantic" matrix `certificate_shadow'[16,1] = 0
    if `"`inject'"' == "mutatesolver" matrix `solver_shadow'[1,1] = 0
    if `"`inject'"' == "mutateoutput" replace `sample_shadow' = 2 in 1

    if `"`inject'"' == "breakpostplugin" {
        di as result "FEPPML_BREAK_ARMED"
        sleep 30000
        exit 3498
    }
    if `"`inject'"' == "postplugin" exit 3498

    capture noisily _fereg_eppml_validate `b_shadow' `v_shadow' `status_shadow' ///
        `fit_shadow' `dof_shadow' `fe_shadow' `cluster_shadow' ///
        `subset_shadow' `separation_shadow' `inference_shadow' ///
        `drop_counts_shadow' `separation_sequence_shadow' ///
        `certificate_shadow' `solver_shadow' `runtime_shadow' `receipt_shadow', ///
        capacity(`coefficient_capacity') ///
        clusters(`n_cluster_terms') ferows(`=`n_absorb_intercepts' + `n_absorb_slopes'') ///
        subsets(`=cond(`n_cluster_terms' == 0, 0, 2^`n_cluster_terms' - 1)') ///
        separations(`separation_count') residual(`=(`"`residual'"' != "")') ///
        saved(`requested_saved_count') ///
        adjustment(`=cond(`"`adjustment_kind'"' == "none", 0, cond(`"`adjustment_kind'"' == "offset", 1, 2))') ///
        vcecode(`=cond(`"`vce_wire'"' == "cluster", 3, 2)') ///
        requestedvcecode(`=cond(`"`vce_request_wire'"' == "unadjusted", 1, cond(`"`vce_request_wire'"' == "cluster", 3, 2))') ///
        weightcode(`=cond(`has_fweight', 1, cond(`has_pweight', 2, 0))') ///
        memorycode(`=cond(`"`memory_mode'"' == "auto", 0, cond(`"`memory_mode'"' == "fast", 1, 2))') ///
        requestedthreads(`threads') feterms(`n_absorb_terms') sources(`n_sources') ///
        methodcodes(`"`separation_codes'"') dropsingletons(`=1 - `keep_singletons'') ///
        requestedsolvercode(`requested_solver_code')
    local validation_rc = _rc
    if `validation_rc' {
        if `validation_rc' == 499 exit 499
        exit 3499
    }
    local coefficient_count = r(coefficients)
    if `has_bias' {
        forvalues row = 1/21 {
            if missing(`bias_diagnostics_shadow'[`row',1]) exit 3499
        }
        if `bias_diagnostics_shadow'[1,1] != cond(`"`bias_method'"' == "exact",1,2) | ///
            `bias_diagnostics_shadow'[2,1] < 2 | `bias_diagnostics_shadow'[2,1] > `receipt_shadow'[1,9] | ///
            `bias_diagnostics_shadow'[3,1] != `biasprobes' | ///
            `bias_diagnostics_shadow'[4,1] != `biasseed' | ///
            `bias_diagnostics_shadow'[5,1] != `biasstream' | ///
            `bias_diagnostics_shadow'[18,1] != 1 | `receipt_shadow'[1,16] != 0 exit 3499
        forvalues effect = 1/2 {
            forvalues col = 1/11 {
                if missing(`bias_support_shadow'[`effect',`col']) exit 3499
            }
            if `bias_support_shadow'[`effect',1] != `fe_shadow'[`effect',4] | ///
                `bias_support_shadow'[`effect',1] < 1 | ///
                `bias_support_shadow'[`effect',2] < 2 | ///
                `bias_support_shadow'[`effect',6] > `bias_diagnostics_shadow'[2,1] | ///
                `bias_support_shadow'[`effect',7] != 0 | `bias_support_shadow'[`effect',8] != 0 | ///
                `bias_support_shadow'[`effect',9] < 1 | ///
                `bias_support_shadow'[`effect',9] > `bias_support_shadow'[`effect',2] | ///
                `bias_support_shadow'[`effect',10] < `bias_support_shadow'[`effect',9] | ///
                `bias_support_shadow'[`effect',10] > `bias_support_shadow'[`effect',6] | ///
                `bias_support_shadow'[`effect',11] <= 0 | `bias_support_shadow'[`effect',11] > 1 exit 3499
            forvalues col = 1/7 {
                if `bias_support_shadow'[`effect',`col'] != floor(`bias_support_shadow'[`effect',`col']) exit 3499
            }
            forvalues col = 2/5 {
                if `bias_support_shadow'[`effect',`col'] > `bias_support_shadow'[`effect',`=`col'+1'] exit 3499
            }
        }
        forvalues slot = 1/`coefficient_count' {
            forvalues row = 1/9 {
                if missing(`bias_shadow'[`row',`slot']) exit 3499
            }
            if reldif(`bias_shadow'[1,`slot'], `b_shadow'[1,`slot']) > `tolerance' | ///
                reldif(`bias_shadow'[4,`slot'], `bias_shadow'[2,`slot'] + `bias_shadow'[3,`slot']) > 1e-12 | ///
                reldif(`bias_shadow'[5,`slot'], `bias_shadow'[1,`slot'] + `bias_shadow'[4,`slot']) > 1e-12 | ///
                min(`bias_shadow'[6,`slot'], `bias_shadow'[7,`slot'], `bias_shadow'[9,`slot']) < 0 exit 3499
        }
    }
    local solver_selected "map"
    if `solver_shadow'[1,2] == 3 local solver_selected "lsmr"
    if `solver_shadow'[1,2] == 7 local solver_selected "cmg"
    local solver_engine "map"
    if `solver_shadow'[1,3] == 2 local solver_engine "lsmr_fused"
    if `solver_shadow'[1,3] == 3 local solver_engine "lsmr_joint"
    if `solver_shadow'[1,3] == 4 local solver_engine "cmg_prepared"
    local solver_effective "map"
    if `solver_shadow'[1,4] == 3 local solver_effective "lsmr"
    if `solver_shadow'[1,4] == 7 local solver_effective "cmg"
    local solver_engine_effective "map"
    if `solver_shadow'[1,5] == 2 local solver_engine_effective "lsmr_fused"
    if `solver_shadow'[1,5] == 3 local solver_engine_effective "lsmr_joint"
    if `solver_shadow'[1,5] == 4 local solver_engine_effective "cmg_prepared"
    local solver_checkpoint "map"
    if `solver_shadow'[1,6] == 3 local solver_checkpoint "lsmr"
    local solver_fallback_reason "none"
    if `solver_shadow'[4,12] == 1 ///
        local solver_fallback_reason "cmg_numerical_nonconvergence"
    if `solver_shadow'[1,12] == 2 ///
        local solver_fallback_reason "checkpoint_lsmr_numerical_nonconvergence"
    if `solver_shadow'[1,12] == 3 | `solver_shadow'[4,12] == 3 ///
        local solver_fallback_reason "cmg_and_checkpoint_numerical_nonconvergence"

    // The native result tables carry enough identity to prove that the
    // rebased FE/cluster topology is still the parser-owned declaration.  Do
    // this comparison outside the generic layout validator because the input
    // metadata matrices remain private to this transaction.
    if `n_absorb_terms' > 0 {
        local fe_cursor 0
        forvalues term = 1/`n_absorb_terms' {
            local categorical_count = `absorb_terms'[`term',2]
            local intercept = `absorb_terms'[`term',3]
            local slopes = `absorb_terms'[`term',4]
            if `intercept' {
                local ++fe_cursor
                local expected_kind = cond(`categorical_count' == 1, 1, 3)
                local expected_source 0
                if `categorical_count' == 1 {
                    forvalues component = 1/`n_absorb_components' {
                        if `absorb_components'[`component',1] == `term' & ///
                           `absorb_components'[`component',2] == 1 & ///
                           `absorb_components'[`component',3] == 1 {
                            local expected_source = `absorb_components'[`component',4]
                        }
                    }
                }
                if `fe_shadow'[`fe_cursor',1] != `expected_kind' | ///
                   `fe_shadow'[`fe_cursor',2] != `term' | ///
                   `fe_shadow'[`fe_cursor',3] != `expected_source' {
                    di as error "PPML fixed-effect intercept identity disagrees with absorb()"
                    exit 3499
                }
            }
            if `slopes' > 0 {
                forvalues slope = 1/`slopes' {
                    local ++fe_cursor
                    local expected_source 0
                    forvalues component = 1/`n_absorb_components' {
                        if `absorb_components'[`component',1] == `term' & ///
                           `absorb_components'[`component',2] == 2 & ///
                           `absorb_components'[`component',3] == `slope' {
                            local expected_source = `absorb_components'[`component',4]
                        }
                    }
                    if `expected_source' == 0 | ///
                       `fe_shadow'[`fe_cursor',1] != 4 | ///
                       `fe_shadow'[`fe_cursor',2] != `term' | ///
                       `fe_shadow'[`fe_cursor',3] != `expected_source' {
                        di as error "PPML fixed-effect slope identity disagrees with absorb()"
                        exit 3499
                    }
                }
            }
        }
        if `fe_cursor' != `n_absorb_intercepts' + `n_absorb_slopes' {
            di as error "PPML fixed-effect channel count disagrees with absorb()"
            exit 3499
        }
    }
    if `n_cluster_terms' > 0 {
        forvalues cluster_row = 1/`n_cluster_terms' {
            if `cluster_shadow'[`cluster_row',2] != ///
               `cluster_terms'[`cluster_row',2] {
                di as error "PPML cluster arity disagrees with vce(cluster ...)"
                exit 3499
            }
        }
    }

    capture assert inlist(`sample_shadow', 0, 1) & !missing(`sample_shadow')
    if _rc {
        di as error "PPML sample output is not a total binary map"
        exit 3499
    }
    capture assert `sample_shadow' <= `touse'
    if _rc {
        di as error "PPML sample output escaped the Stata candidate sample"
        exit 3499
    }
    quietly count if `sample_shadow'
    if r(N) != `receipt_shadow'[1,9] {
        di as error "PPML sample output disagrees with the committed final row count"
        exit 3499
    }
    foreach shadow in `residual_shadow' `xb_shadow' `d_shadow' ///
        `eta_shadow' `mu_shadow' `saved_shadow_variables' `bias_shadow_variables' {
        if `"`shadow'"' == "" continue
        capture assert `shadow' < . if `sample_shadow'
        if _rc {
            di as error "PPML observation output contains a missing final-sample value"
            exit 3499
        }
        capture assert missing(`shadow') if !`sample_shadow'
        if _rc {
            di as error "PPML observation output was published outside the final sample"
            exit 3499
        }
    }
    if `"`mu'"' != "" {
        capture assert `mu_shadow' > 0 if `sample_shadow'
        if _rc {
            di as error "PPML mean output is not strictly positive"
            exit 3499
        }
    }
    if `"`dropreason'"' != "" {
        capture assert `dropreason_shadow' == int(`dropreason_shadow') & ///
            inrange(`dropreason_shadow', 0, 13)
        if _rc {
            di as error "PPML drop-reason output is invalid"
            exit 3499
        }
        capture assert (`dropreason_shadow' == 0) == `sample_shadow'
        if _rc {
            di as error "PPML drop reasons disagree with the final sample"
            exit 3499
        }
        forvalues drop_reason = 0/13 {
            quietly count if `dropreason_shadow' == `drop_reason'
            if r(N) != `drop_counts_shadow'[`=`drop_reason' + 1',2] {
                di as error "PPML drop-reason output disagrees with its bounded count table"
                exit 3499
            }
        }
    }

    if `coefficient_count' < `matrix_dimension' {
        matrix `b_shadow' = `b_shadow'[1, 1..`coefficient_count']
        matrix `v_shadow' = `v_shadow'[1..`coefficient_count', 1..`coefficient_count']
        matrix `status_shadow' = `status_shadow'[1..`coefficient_count', 1..4]
    }

    // Coefficient identity belongs to the original parser-owned model axis.
    // A factor level may vanish during domain filtering or separation while
    // its base/omission slot remains part of the certified result.
    local posted_names `"`coefficient_names' _cons"'
    local posted_names = strtrim(`"`posted_names'"')
    local posted_name_count : word count `posted_names'
    if `posted_name_count' != `coefficient_count' {
        di as error "PPML coefficient-name count disagrees with receipt"
        exit 3499
    }
    matrix colnames `b_shadow' = `posted_names'
    matrix colnames `v_shadow' = `posted_names'
    matrix rownames `v_shadow' = `posted_names'
    tempname marker_b marker_v posted_b posted_v
    matrix `marker_b' = J(1, `coefficient_count', 1)
    matrix `marker_v' = I(`coefficient_count')
    forvalues slot = 1/`coefficient_count' {
        if `status_shadow'[`slot',2] != 1 {
            matrix `marker_b'[1,`slot'] = 0
            matrix `marker_v'[`slot',`slot'] = 0
        }
    }
    matrix colnames `marker_b' = `posted_names'
    matrix colnames `marker_v' = `posted_names'
    matrix rownames `marker_v' = `posted_names'
    _ms_findomitted `marker_b' `marker_v'
    local marked_names : colfullnames `marker_b'
    mata: st_matrix("`posted_b'", st_matrix("`b_shadow'"))
    mata: st_matrix("`posted_v'", st_matrix("`v_shadow'"))
    matrix colnames `posted_b' = `marked_names'
    matrix colnames `posted_v' = `marked_names'
    matrix rownames `posted_v' = `marked_names'
    if `"`inject'"' == "poststripe" exit 3498

    local N_effective = `fit_shadow'[17,1]
    local eweight
    if `has_fweight' local eweight "[fweight=`fweight']"
    if `has_pweight' local eweight "[pweight=`pweight']"
    ereturn post `posted_b' `posted_v' `eweight', depname(`depvar') ///
        obs(`N_effective') esample(`sample_shadow') buildfvinfo
    if `"`inject'"' == "postereturn" exit 3498

    mata: _fereg_ppml_post_scalars()
    if `"`inject'"' == "postscalars" exit 3498

    if `has_bias' {
        matrix colnames `bias_shadow' = `marked_names'
        matrix rownames `bias_shadow' = raw derivative curvature total corrected mcse_d mcse_c cov_dc mcse_total
        local bias_matrices b_bias_raw b_bias_d b_bias_c b_bias_total b_bc bias_mcse_d bias_mcse_c bias_cov_dc bias_mcse_total
        tempname bias_row
        forvalues row = 1/9 {
            local name : word `row' of `bias_matrices'
            matrix `bias_row' = `bias_shadow'[`row',1..`coefficient_count']
            ereturn matrix `name' = `bias_row', copy
        }
        matrix rownames `bias_diagnostics_shadow' = method units probes seed stream groups solves ///
            max_score max_reconstruction seconds workspace_bytes max_increment ///
            min_mu max_mu min_unit_rows max_unit_rows route valid ///
            tolerance min_level_information max_fe_leverage_lb
        matrix colnames `bias_diagnostics_shadow' = value
        ereturn matrix bias_diagnostics = `bias_diagnostics_shadow', copy
        matrix rownames `bias_support_shadow' = fe1 fe2
        matrix colnames `bias_support_shadow' = levels min_units p25_units median_units p75_units max_units ///
            single_unit_levels single_unit_fraction min_info_units median_info_units max_unit_info_share
        ereturn matrix bias_support = `bias_support_shadow', copy
        ereturn local bias_method "`bias_method'"
        ereturn local bias_unit "`biasunit'"
        ereturn local bias_rng "philox4x32-10"
        ereturn local bias_normalization "minimum-second-level-per-component"
        ereturn local bias_validity "interior-retained-sample"
        ereturn local bias_fevars "`bias_target_variables'"
    }

    matrix colnames `status_shadow' = slot status kept omitted
    matrix rownames `status_shadow' = `marked_names'
    matrix rownames `fit_shadow' = N_physical rank_finite rank_absorbed ///
        df_structural df_reported deviance ll ll_0 r2_p mu_0 chi2 ///
        df_chi2 sum_y outer_iterations inner_iterations step_halvings ///
        N_effective sumweights raw_sumweights weight_multiplier ///
        weight_code N_candidate N_original converged log_raw_sumweights ///
        log_weight_multiplier objective_scale_code
    matrix colnames `fit_shadow' = value
    matrix rownames `dof_shadow' = N_effective rank_finite rank_absorbed ///
        rank_model df_structural df_reported N_clust components ///
        rank_approximate N_physical N_clustervars df_cluster_cap
    matrix colnames `dof_shadow' = value
    matrix colnames `fe_shadow' = kind term source categories redundant rank ///
        approximate nested
    local fe_row_names
    forvalues row = 1/`fe_rows' {
        local fe_row_names `fe_row_names' fe`row'
    }
    matrix rownames `fe_shadow' = `fe_row_names'
    matrix colnames `cluster_shadow' = dimension source_count groups reserved
    local cluster_row_names
    forvalues row = 1/`cluster_rows' {
        local cluster_row_names `cluster_row_names' cluster`row'
    }
    matrix rownames `cluster_shadow' = `cluster_row_names'
    matrix colnames `subset_shadow' = mask groups sign cardinality
    local subset_row_names
    forvalues row = 1/`subset_rows' {
        local subset_mask = `subset_shadow'[`row',1]
        local subset_row_names `subset_row_names' subset`subset_mask'
    }
    matrix rownames `subset_shadow' = `subset_row_names'
    matrix colnames `separation_shadow' = method status executions generation ///
        iterations direct_drops singleton_drops certificates interior_error ///
        boundary_violation span_residual negative_margin checked_effects zero_levels
    local separation_row_names
    forvalues row = 1/`separation_rows' {
        local method_code = `separation_shadow'[`row',1]
        if `method_code' == 1 local method_name "singleton"
        else if `method_code' == 2 local method_name "fe"
        else if `method_code' == 3 local method_name "simplex"
        else if `method_code' == 4 local method_name "ir"
        else local method_name "mu"
        local separation_row_names `separation_row_names' `method_name'
    }
    matrix rownames `separation_shadow' = `separation_row_names'
    matrix rownames `inference_shadow' = requested_vce_code ///
        effective_vce_code correction correction_policy_code ///
        coefficient_distribution_code psd_state psd_adjusted ///
        psd_clipped_eigenvalues psd_raw_minimum_eigenvalue ///
        psd_final_minimum_eigenvalue psd_final_rank ///
        psd_eigensystem_recon_relinf psd_orthogonality_relinf ///
        psd_clipped_recon_relinf certificate_tolerance psd_passed
    matrix colnames `inference_shadow' = value
    matrix rownames `drop_counts_shadow' = retained excluded_if_in ///
        missing_outcome missing_regressor invalid_or_missing_weight ///
        missing_fixed_effect missing_cluster missing_adjustment singleton ///
        fixed_effect_separation simplex_separation ir_separation ///
        mu_separation other_model_exclusion
    matrix colnames `drop_counts_shadow' = reason count
    matrix rownames `separation_sequence_shadow' = singleton fe simplex ir mu
    matrix colnames `separation_sequence_shadow' = method requested ///
        registered_order executions first_trace_row last_trace_row ///
        status_mask total_iterations direct_drops singleton_drops ///
        first_generation last_generation all_certificates_passed
    matrix rownames `certificate_shadow' = score_finite_absinf score_fe_absinf ///
        score_relinf score_rel2 linear_within_relinf linear_reconstruct_relinf ///
        predictor_reconstruct_relinf mean_reconstruct_relinf ///
        saved_total_relinf saved_channels_relinf bread_symmetry_relinf ///
        meat_relinf covariance_relinf covariance_symmetry_relinf ///
        wald_solve_relinf composite_passed
    matrix colnames `certificate_shadow' = value
    matrix rownames `solver_shadow' = map lsmr_fused lsmr_joint cmg_prepared
    matrix colnames `solver_shadow' = requested_family selected_family ///
        selected_engine effective_family effective_engine checkpoint_family ///
        row_engine ordinary_frames checkpoint_frames retries fallback_count ///
        fallback_reason iterations setup_seconds solve_seconds ///
        certification_seconds absinf_max relinf_max rel2_max ///
        normal_equation_max projection_absinf_max backward_error_max
    matrix rownames `runtime_shadow' = total_seconds outer_iterations ///
        inner_iterations step_halvings memory_requested memory_selected ///
        linear_route fallback_mask fallback_count threads_requested ///
        threads_effective modeled_fast_bytes modeled_low_bytes ///
        modeled_selected_bytes modeled_scratch_bytes observed_scratch_bytes ///
        allocation_previous allocation_start allocation_current allocation_peak ///
        allocation_peak_delta allocation_growth allocation_model_ratio ///
        workspace_allocation_free iteration_allocation_free diagnostic_records ///
        strict_profile_rejections time_preparation time_separation_fe ///
        time_separation_simplex time_separation_ir time_separation_mu ///
        time_weight_response_refresh time_fe_projection time_finite_solve ///
        time_objective_line_search time_strict_final_solve time_inference ///
        time_fe_recovery time_publication allocation_audit_present ///
        audit_iterations audit_events_start audit_events_end ///
        audit_events_delta audit_current_start audit_current_end ///
        audit_current_growth audit_peak_start audit_peak_end ///
        audit_peak_growth ppml_obs_capacity_start ppml_coef_capacity_start ///
        ppml_diag_capacity_start ppml_obs_capacity_end ppml_coef_capacity_end ///
        ppml_diag_capacity_end ppml_capacity_stable ///
        linear_element_capacity_start linear_resident_elements_start ///
        linear_scratch_bytes_start linear_fe_topology_bytes_start ///
        linear_element_capacity_end linear_resident_elements_end ///
        linear_scratch_bytes_end linear_fe_topology_bytes_end ///
        linear_capacity_stable audit_scratch_bytes_start ///
        audit_scratch_bytes_end audit_scratch_stable audit_linear_route ///
        audit_route_qualified linear_session_allocation_free ///
        fresh_warm_equivalent steady_iteration_median_ns ///
        allocation_certificate_passed two_consecutive_convergence ///
        rank_frozen repeated_baseline_growth action_events_start ///
        action_events_end action_events_delta modeled_peak_prep ///
        modeled_peak_sep_fe modeled_peak_sep_simplex modeled_peak_sep_ir ///
        modeled_peak_sep_mu modeled_peak_weight_refresh ///
        modeled_peak_fe_projection modeled_peak_finite_solve ///
        modeled_peak_line_search modeled_peak_final_solve ///
        modeled_peak_inference modeled_peak_fe_recovery ///
        modeled_peak_publication observed_peak_prep observed_peak_sep_fe ///
        observed_peak_sep_simplex observed_peak_sep_ir observed_peak_sep_mu ///
        observed_peak_weight_refresh observed_peak_fe_projection ///
        observed_peak_finite_solve observed_peak_line_search ///
        observed_peak_final_solve observed_peak_inference ///
        observed_peak_fe_recovery observed_peak_publication
    matrix colnames `runtime_shadow' = value
    matrix colnames `receipt_shadow' = magic major minor vce coefficients ///
        covariance_dimension N_original N_candidate N_final cluster_dimensions ///
        fixed_effect_rows subset_rows sample_present residual_present ///
        certificates_passed separated separation_rows saved_channels adjustment ///
        inference_rows drop_count_rows sequence_rows separation_executions ///
        separation_generations initial_hash_hi initial_hash_lo final_hash_hi ///
        final_hash_lo trace_digest_hi trace_digest_lo hash_digest_hi ///
        hash_digest_lo complete
    matrix rownames `receipt_shadow' = receipt

    ereturn matrix feppml_status = `status_shadow', copy
    ereturn matrix feppml_fit = `fit_shadow', copy
    ereturn matrix feppml_dof = `dof_shadow', copy
    ereturn matrix feppml_fe = `fe_shadow', copy
    ereturn matrix feppml_cluster = `cluster_shadow', copy
    ereturn matrix feppml_subset = `subset_shadow', copy
    ereturn matrix feppml_separation = `separation_shadow', copy
    ereturn matrix feppml_inference = `inference_shadow', copy
    ereturn matrix feppml_drop_counts = `drop_counts_shadow', copy
    ereturn matrix feppml_separation_sequence = `separation_sequence_shadow', copy
    ereturn matrix feppml_certificate = `certificate_shadow', copy
    ereturn matrix feppml_solver = `solver_shadow', copy
    ereturn matrix feppml_runtime = `runtime_shadow', copy
    matrix rownames `memory_shadow' = schema_version estimate_status ///
        stata_used_before stata_alloc_before native_live_before native_peak_est ///
        stata_growth_est combined_peak_est additional_reserve safety_headroom model_gap_bits
    matrix colnames `memory_shadow' = present value
    ereturn matrix feppml_memory = `memory_shadow', copy
    ereturn local feppml_memory_method "phase-capacity-partial-v2"
    if `memory_shadow'[2,2] == 2 ereturn local feppml_memory_method "phase-capacity-calibrated-v2"
    ereturn matrix feppml_receipt = `receipt_shadow', copy
    if `n_absorb_intercepts' + `n_absorb_slopes' > 0 {
        tempname posted_dof_table
        local actual_fe_rows = `n_absorb_intercepts' + `n_absorb_slopes'
        matrix `posted_dof_table' = J(`actual_fe_rows', 5, 0)
        local dof_row_names
        forvalues row = 1/`actual_fe_rows' {
            matrix `posted_dof_table'[`row',1] = `fe_shadow'[`row',4]
            matrix `posted_dof_table'[`row',2] = `fe_shadow'[`row',5]
            matrix `posted_dof_table'[`row',3] = `fe_shadow'[`row',6]
            matrix `posted_dof_table'[`row',4] = `fe_shadow'[`row',7]
            matrix `posted_dof_table'[`row',5] = `fe_shadow'[`row',8]
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
            local dof_row_names `"`dof_row_names' `dof_row_name'"'
        }
        matrix rownames `posted_dof_table' = `dof_row_names'
        mata: st_matrixcolstripe("`posted_dof_table'", ///
            (J(5,1,""), ("Categories" \ "Redundant" \ "Num Coefs" \ ///
            "Inexact?" \ "Nested?")))
        ereturn matrix dof_table = `posted_dof_table', copy
    }
    local intended_indepvars
    if `n_terms' > 0 {
        forvalues slot = 1/`n_terms' {
            if `status_shadow'[`slot',2] == 2 continue
            local name : word `slot' of `coefficient_names'
            local remaining `"`name'"'
            local intended_name
            while `"`remaining'"' != "" {
                gettoken component remaining : remaining, parse("#")
                if `"`component'"' == "#" {
                    local intended_name `"`intended_name'#"'
                    continue
                }
                local intended_component `"`component'"'
                if substr(`"`component'"', 1, 2) == "o." {
                    local intended_component = substr(`"`component'"', 3, .)
                }
                else if regexm(`"`component'"', "^(-?[0-9]+)(b|bn|o)?[.](.+)$") {
                    local factor_level = regexs(1)
                    local factor_variable = regexs(3)
                    local intended_component `"`factor_level'bn.`factor_variable'"'
                }
                local intended_name `"`intended_name'`intended_component'"'
            }
            local intended_indepvars `"`intended_indepvars' `intended_name'"'
        }
    }
    local intended_indepvars = strtrim(`"`intended_indepvars'"')
    if `"`publicmode'"' == "" {
        ereturn local cmd "_fereg_estimate_ppml"
        ereturn local cmdline `"`hidden_cmdline'"'
    }
    else {
        ereturn local cmd "feppml"
        ereturn local cmdline `"`publiccmdline'"'
        ereturn local feppml_gate "public_ppml_v1"
        ereturn local feppml_memory_requested "`memory_mode'"
        ereturn scalar feppml_poolsize = `poolsize'
        ereturn scalar feppml_requested_tolerance = `tolerance'
        ereturn scalar feppml_iterate = `iterate'
        ereturn scalar feppml_timeit = (`"`publictimeit'"' != "")
    }
    ereturn local model "poisson"
    ereturn local depvar "`depvar'"
    ereturn local indepvars `"`intended_indepvars'"'
    ereturn local vce `"`vce_wire'"'
    ereturn local vcetype "Robust"
    ereturn local absvars `"`absorb_labels'"'
    ereturn local extended_absvars `"`absorb_labels'"'
    ereturn local separation `"`separation_wire'"'
    ereturn local memory `"`memory_mode'"'
    ereturn local chi2type "Wald"
    ereturn local title "Poisson regression with high-dimensional fixed effects"
    if `n_absorb_terms' > 0 {
        ereturn local title2 "Absorbing `n_absorb_terms' HDFE group"
    }
    ereturn scalar poolsize = `poolsize'
    ereturn scalar requested_threads = `threads'
    ereturn scalar deterministic = `deterministic_wire'
    ereturn local adjustment `"`adjustment_kind'"'
    if `"`offset'"' != "" ereturn local offset "`offset'"
    if `"`exposure'"' != "" ereturn local exposure "`exposure'"
    if `has_fweight' {
        ereturn local wtype "fweight"
        if `"`publicmode'"' != "" & strtrim(`"`publicwexp'"') != "" {
            ereturn local wexp `"`publicwexp'"'
        }
        else ereturn local wexp "= `fweight'"
    }
    if `has_pweight' {
        ereturn local wtype "pweight"
        if `"`publicmode'"' != "" & strtrim(`"`publicwexp'"') != "" {
            ereturn local wexp `"`publicwexp'"'
        }
        else ereturn local wexp "= `pweight'"
    }
    if `"`vce_wire'"' == "cluster" {
        local posted_cluster_labels = strtrim(`"`cluster_labels'"')
        local minimum_clusters .
        forvalues row = 1/`n_cluster_terms' {
            local groups = `cluster_shadow'[`row',3]
            if missing(`minimum_clusters') | `groups' < `minimum_clusters' {
                local minimum_clusters = `groups'
            }
            local cluster_label : word `row' of `cluster_labels'
            ereturn scalar N_clust`row' = `groups'
            ereturn local clustvar`row' `"`cluster_label'"'
        }
        ereturn scalar N_clust = `minimum_clusters'
        ereturn scalar N_clustervars = `n_cluster_terms'
        ereturn local clustvar `"`posted_cluster_labels'"'
        ereturn local title3 "Statistics robust to heteroskedasticity and clustering"
    }
    if `"`residual'"' != "" {
        ereturn local resid "`residual'"
        ereturn local residual_type "`residual_wire'"
    }
    if `"`xb'"' != "" ereturn local xb_var "`xb'"
    if `"`d'"' != "" ereturn local d_var "`d'"
    if `"`eta_target'"' != "" ereturn local eta_var "`eta_target'"
    if `"`mu'"' != "" ereturn local mu_var "`mu'"
    if `"`dropreason'"' != "" ereturn local drop_reason_var "`dropreason'"
    ereturn local predict "feppml_p"
    ereturn local solver "`solver_mode'"
    ereturn local feppml_solver_requested "`solver_mode'"
    ereturn local feppml_solver_selected "`solver_selected'"
    ereturn local feppml_solver_engine "`solver_engine'"
    ereturn local feppml_solver_effective "`solver_effective'"
    ereturn local feppml_solver_engine_effective "`solver_engine_effective'"
    ereturn local feppml_solver_checkpoint "`solver_checkpoint'"
    ereturn local feppml_solver_fallback_reason "`solver_fallback_reason'"
    ereturn local feppml_action_protocol "1.6"
    ereturn local feppml_model_wire "2.1"
    ereturn local feppml_request_wire "1.8"
    ereturn local feppml_result_wire "1.5"
    ereturn local feppml_plugin "0.5.0"

    if `"`inject'"' == "breakpostpublic" {
        di as result "FEPPML_BREAK_ARMED"
        sleep 30000
        exit 3498
    }
    if `"`inject'"' == "postpublic" exit 3498
    if `"`inject'"' == "postdiagnostics" exit 3498
    if `"`inject'"' == "precommit" exit 3498

    nobreak {
        if `"`residual'"' != "" rename `residual_shadow' `residual'
        if `"`xb'"' != "" rename `xb_shadow' `xb'
        if `"`d'"' != "" rename `d_shadow' `d'
        if `"`eta_target'"' != "" rename `eta_shadow' `eta_target'
        if `"`mu'"' != "" rename `mu_shadow' `mu'
        if `"`dropreason'"' != "" rename `dropreason_shadow' `dropreason'
        local n_saved_targets : word count `saved_target_variables'
        if `n_saved_targets' > 0 {
            forvalues saved_index = 1/`n_saved_targets' {
                local saved_shadow : word `saved_index' of `saved_shadow_variables'
                local saved_target : word `saved_index' of `saved_target_variables'
                rename `saved_shadow' `saved_target'
            }
        }
        if `output_bias_count' {
            forvalues index = 1/`output_bias_count' {
                local shadow : word `index' of `bias_shadow_variables'
                local target : word `index' of `bias_target_variables'
                rename `shadow' `target'
            }
        }
        _estimates unhold `held_estimates', not
        if `"`publictimeit'"' != "" {
            di as txt "feppml native total-to-commit (seconds): " ///
                as result %9.4f `runtime_shadow'[1,1]
            di as txt "feppml action memory: partial estimate; combined peak unavailable"
        }
        if `"`publicdisplay'"' != "" ereturn display
    }
end

program define _fereg_eppml_validate, rclass
    version 18.0
    syntax namelist(min=16 max=16), CAPACITY(integer) CLUSTERS(integer) ///
        FEROWS(integer) SUBSETS(integer) SEPARATIONS(integer) ///
        RESIDUAL(integer) SAVED(integer) ADJUSTMENT(integer) VCECODE(integer) ///
        REQUESTEDVCECODE(integer) DROPSINGLETONS(integer) ///
        WEIGHTCODE(integer) MEMORYCODE(integer) REQUESTEDTHREADS(integer) ///
        FETERMS(integer) SOURCES(integer) METHODCodes(string) ///
        REQUESTEDSOLVERCODE(integer)
    local b : word 1 of `namelist'
    local V : word 2 of `namelist'
    local status : word 3 of `namelist'
    local fit : word 4 of `namelist'
    local dof : word 5 of `namelist'
    local fe : word 6 of `namelist'
    local cluster : word 7 of `namelist'
    local subset : word 8 of `namelist'
    local separation : word 9 of `namelist'
    local inference : word 10 of `namelist'
    local drop_counts : word 11 of `namelist'
    local separation_sequence : word 12 of `namelist'
    local certificate : word 13 of `namelist'
    local solver : word 14 of `namelist'
    local runtime : word 15 of `namelist'
    local receipt : word 16 of `namelist'
    // Package-owned Stata 18 object code autoloads without first-fit compilation.
    mata: st_local("validation_rc", strofreal(_fereg_ppml_validate_result()))
    if `validation_rc' exit `validation_rc'
    return scalar coefficients = `receipt'[1,5]
end
